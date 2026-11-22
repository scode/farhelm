# Podman leftover discovery and deletion run SSH commands on unused places without ssh_route_lock

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

Opening Settings → Containers (which checks for leftover containers on every observed host) or deleting an unregistered
leftover container while turning off Tailscale mode or removing an unused SSH host can race with key cleanup and master
shutdown, causing the leftover operation to fail or leaving orphaned Tailscale key files and shared SSH masters behind.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman_probe.rs:730-745`.

`PodmanPlaces::refresh` (`crates/farhelm-helm/src/environment/podman_probe.rs:796-813`) acquires
`manager.ssh_route_lock(host).read_owned().await` so a place check cannot start an SSH master or write private Tailscale
keys concurrently with `hosts::set_tailscale` (`hosts.rs:993-1030`) or `ProvisioningService::delete_registered_host`
(`service.rs:2582-2597`). However, two leftover operations run on ordinary SSH hosts that own zero recorded containers
or recipe artifacts (and therefore pass `podman::refuse_referenced_place` during `set_tailscale` or removal) without
acquiring `ssh_route_lock(host)`: (1) `Podman::leftovers` (`crates/farhelm-helm/src/environment/podman.rs:886-905`)
scans every host with `runtime_observed(row) == true` via `self.list_owned` -> `Podman::command` (`podman.rs:242-260`)
-> `PodmanPlaces::command` (`podman_probe.rs:730-745`); and (2) `Podman::delete_leftover`
(`crates/farhelm-helm/src/environment/podman.rs:832-864`) runs `self.inspect` and `self.remove_container` through the
same `Podman::command` -> `PodmanPlaces::command` path. On a cached probe hit, `PodmanPlaces::runtime`
(`podman_probe.rs:630-671`) returns immediately without invoking `refresh`, so `SystemPlaceCommands::execute`
(`podman_probe.rs:288-295`) invokes `TailscaleSshAccess::prepare` and spawns `ssh` with `ControlMaster=auto` completely
outside `ssh_route_lock(host)`. Concurrently, `set_tailscale` (toggling Tailscale off) or `delete_registered_host` can
take `ssh_route_lock(host)` exclusively, retire existing masters via `release_shared_connections(&old)`, commit the
edit, and delete `<state>/tailscale/<digest>` via `forget_tailscale_keys(&old)` while the leftover command either loses
its `known_hosts` file mid-launch or recreates the pin directory and a new `/tmp/fhs-<scope>` ControlMaster afterward.
(By contrast, `PodmanPlaces::creation_proxy` at `podman_probe.rs:719-726` runs only after a session container row
already references the place host.) Fix direction: acquire `ssh_route_lock(host).read_owned()` around place-scoped SSH
commands in `Podman::command` (for `HostKind::Ssh` places) and re-check the current route before running
`SystemPlaceCommands::execute`.

Reviewer metadata (g45-tailscale-provisioning-podman/f4): suggested bucket: other (not highest: does not cross modes or
expose credentials/data because Tailscale uses a distinct `/tmp/fhs-<scope>` socket namespace, but can fail a leftovers
listing/deletion or leave orphaned Tailscale pins/masters behind); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g45-tailscale-provisioning-podman/f4`.

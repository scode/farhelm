# Removing or uninstalling an ordinary SSH host blocks on in-flight probes and SSH sign-in prompts

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

Removing an ordinary SSH host (whether or not Tailscale mode is enabled) can hang past the 20-second timeout and leave
the host stuck as busy for minutes whenever a background Podman check (started automatically when opening the launcher),
a setup/update/uninstall check, or an open SSH sign-in prompt is running for that host. Finishing an uninstall can stall
on its final forget-host step for the same reason.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/service.rs:2582-2586`.

In `crates/farhelm-helm/src/provisioning/service.rs:2582-2586`, `ProvisioningService::delete_registered_host`
unconditionally awaits `self.manager.ssh_route_lock(host).write_owned().await` for every `HostKind::Ssh` row while its
caller holds `host_provision_lock(host)`. Unlike `hosts::set_tailscale` (`crates/farhelm-helm/src/hosts.rs:993-997`),
which uses `try_write_owned()` and returns `HostBusy` promptly, `delete_registered_host` waits for all `ssh_route_lock`
readers to finish. Three readers hold `ssh_route_lock(host).read_owned()` across long remote/interactive waits without
holding `host_provision_lock`: (1) `SetupSignIn::start`
(`crates/farhelm-helm/src/provisioning/setup_sign_in.rs:147-164`) holds it for the entire `setup.wait(...)`, where
`InteractiveChild::run` (`crates/farhelm-helm/src/ssh_sign_in.rs:23,420`) waits up to 120 seconds per interactive prompt
and registered attempts have no cancellation path (`cancel` is `None` for non-drafts at `setup_sign_in.rs:125`); (2)
`PodmanPlaces::refresh` (`crates/farhelm-helm/src/environment/podman_probe.rs:796-813`) holds it across the entire
multi-command `probe(...)` (each command bounded at 20s in `podman_probe.rs:227`, triggered automatically when the
launcher opens); and (3) `ProvisioningService::lock_ssh_plan`
(`crates/farhelm-helm/src/provisioning/service.rs:3718-3734`) holds it across `probe_owned`, `plan_update_owned`, and
`plan_uninstall_owned`. This directly violates `SPEC.md`, "Waiting between operations on one host" (lines 5170-5178):
"removing a host, which must respond promptly whatever that host is doing, if only to refuse because it is busy."
Consequences: (a) `DELETE /api/hosts/{id}` (`hosts.rs:1286-1304`) hits the 20s `HOST_REPLY_BOUND` and returns 504 while
leaving `host_provision_lock(host)` held for up to 120+ seconds, turning subsequent host operations into 409 Busy; (b)
`hosts::remove_host_checked` (`hosts.rs:1337-1347`, agent-approved removal, which has no `within_host_reply_bound`
wrapper per `hosts.rs:1263-1265`) stalls until the reader completes; (c) Uninstall's final
`ProvisioningAction::ForgetHost` (`service.rs:3469-3481`) stalls after already removing the remote supervisor if a
background Podman probe or planning check holds the read lock; and (d) if an SSH sign-in prompt was open when the user
chose Remove, the prompt stays open instead of being cancelled. Verification/fix direction: For `remove_host` /
`remove_host_checked`, either use `try_write_owned()` on `ssh_route_lock` (refusing promptly with 409 Busy as
`set_tailscale` does and as `SPEC.md` allows) or cancel/fence in-flight registered setup sign-in and probe operations on
deletion; for Uninstall's `ForgetHost`, note that Uninstall cannot simply fail with Busy after already removing the
supervisor without leaving the host stuck until retry, so cancelling/fencing readers or ensuring `ForgetHost` cannot be
blocked by a 120s interactive sign-in is important.

Reviewer metadata (g45-tailscale-provisioning-podman/f1): suggested bucket: high (not highest: stalls removal/uninstall
and holds the host busy, but does not cause data loss, credential exposure, or a security boundary bypass); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g45-tailscale-provisioning-podman/f1`.

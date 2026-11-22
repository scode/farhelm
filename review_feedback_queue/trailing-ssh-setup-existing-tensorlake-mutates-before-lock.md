# Confirmed SSH-destination setup on an existing Tensorlake host mutates coordinates before locking or checking suspended

Reviewed commit: dcebd0aa9beb70b8b29ce309a2df600a05dc64f3

## TLDR

Confirming a setup plan for a sandbox that became suspended or entered a lifecycle change after planning can overwrite
its saved Farhelm paths without taking its host lock and bypass the check that prevents dialing a suspended sandbox.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/service.rs:1542`.

Turn-2 refinement confirming exact scope. In `crates/farhelm-helm/src/provisioning/service.rs:1495-1572`,
`ProvisioningService::claim_register_and_start` handles confirmed ADD plans (`start_add`). For an existing Tensorlake
host probed via `ProbeDestination::Ssh` (`ProbeRegistration::Ssh`), it checks `known_owner_refusal` (`service.rs:1532`)
and inserts into `memory.busy` (`service.rs:1536`), then calls
`self.register(registration, Some(&pending.plan), None, None)` at `service.rs:1543` BEFORE `launch_claimed_run` acquires
`host_provision_lock(host)` (`service.rs:2502`) and BEFORE `revalidate` runs `self.require_not_suspended(&row)`
(`service.rs:2770`). Inside `register()`, `register_probed_ssh_host_with_mode` (`store.rs:7751-7761`) updates
`remote_farhelm` and `remote_state_dir` in `helm.db`, `sync_registry()` (`service.rs:1369`) stops and replaces the
connection actor whenever either path changed (`manager.rs:1655-1660` `reconfigured()`), and lines 1379-1391 call
`retry_for_provisioning`/`retry_now`. Replacing the actor resets `HostState::Suspended` / `Stopped` /
`TensorlakeChanging` to `HostState::Connecting`, bypassing `require_not_suspended(&row)` (`service.rs:2770, 3082`),
racing any concurrent lifecycle operation holding `host_provision_lock`, and leaving uninstalled paths persisted in
`helm.db`. Scope check: `start_update_owned` (`service.rs:1875-1908`) is NOT affected (it takes
`try_host_provision_lock` before claiming and never calls `register()` before `revalidate` at line 2905), and Podman
`ProbeRegistration::ExistingEnvironment` is only constructed by `registration_for_row` (`service.rs:3825`) for
Update/Uninstall/post-registration, not by `probe()`. Commit `dcebd0aa` masked this in
`crates/farhelm-helm/src/provisioning.rs:4906-4920` (`a_plan_made_while_running_does_not_run_once_suspended`) by
pre-writing `plan.farhelm_path`/`plan.state_dir` via `admit_tensorlake_install` + `sync_registry()` before suspending so
`reconfigured()` stays false. Fix direction: take/check `host_provision_lock` (or check `suspended_refusal` under the
lock) before calling `register()` in `claim_register_and_start`, and keep a test where the plan's coordinates differ
from the stored row while suspended.

Reviewer metadata (g08-tensorlake-specs-and-tests/l3): suggested bucket: other (cannot be highest: does not lose user
data, credentials, processes, or cross a security boundary; dialing a suspended sandbox fails or hangs an SSH probe and
can leave uninstalled coordinates written to helm.db); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g08-tensorlake-specs-and-tests/l3`.

# Refused setup confirmation still leaves planned install paths persisted on a retargeted or mode-changed SSH host

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

Confirming Setup for an already-registered SSH host writes the plan's remote Farhelm binary and state-directory paths
into the host's database row before taking the host's provisioning lock. If a concurrent destination change or Tailscale
mode change wins that lock, Farhelm refuses to start the setup run, yet leaves the refused plan's un-installed paths
saved on the host. The host then keeps trying to dial the new address with the old machine's planned binary and state
paths (for example a macOS home path on a Linux host) and fails to connect.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/service.rs:1588`.

In `crates/farhelm-helm/src/provisioning/service.rs` at `1b3fe49af86979aebc2719965682a17f8c6366ac`
(`claim_register_and_start`, lines 1536-1618; lines 1525-1607 at `1ec7724e3cd0e8353c75af6f9b65842795518e9d`), confirming
an ADD plan for an existing host inserts the host into `memory.busy` (line 1582) and calls
`self.register(registration, Some(&pending.plan), None, None)` (line 1589) BEFORE `start_run` / `launch_claimed_run`
acquires `self.manager.host_provision_lock(host)` (line 2550; line 2537 at `1ec7724e3cd0e8353c75af6f9b65842795518e9d`).
`register()` executes `HelmStore::register_probed_ssh_host_with_mode` (`crates/farhelm-helm/src/store.rs:7787-7797`),
durably overwriting `remote_farhelm` and `remote_state_dir` with the plan's target paths, and reconciles the actor
(`sync_registry` + `retry_for_provisioning`). Because `host_provision_lock` is not held during `register()`, a
concurrent `retarget` (`crates/farhelm-helm/src/hosts.rs:928-971`) or `set_tailscale` (`hosts.rs:987-1038`) can either
already hold `host_provision_lock` while `register()` mutates the row or acquire it immediately after `register()`
commits. `update_ssh_destination` (`store.rs:8157`) only updates `destination` and preserves `remote_farhelm` and
`remote_state_dir`. When `launch_claimed_run` then acquires `host_provision_lock` and checks `row.destination` /
`row.tailscale` (lines 2552-2567; lines 2540-2555 at `1ec7724e3cd0e8353c75af6f9b65842795518e9d`), it returns
`Err(HostStoreError::StaleAttempt)` or `Err(HostStoreError::SshModeChanged)` and cancels the run, leaving the
un-installed paths from the refused plan persisted on the host row. Notably, the new regression test
`provisioning_refuses_retarget_after_the_setup_confirmation_write` (`crates/farhelm-helm/src/provisioning.rs:2934-2985`)
drives this exact interleaving, asserts `stored_row.remote_farhelm.is_some()` before retargeting to `after.example`, and
never checks that `after.example` was not left with `before.example`'s planned paths.

How to verify: Inspect `stored_row(&harness.store, host).await` at the end of
`provisioning_refuses_retarget_after_the_setup_confirmation_write` — both `remote_farhelm` and `remote_state_dir` remain
populated with `before.example`'s un-installed plan paths even though the confirmation was refused with `StaleAttempt`.

Fix: When `existing` is `Some(host)` in `claim_register_and_start`, acquire `host_provision_lock(existing)` (either
waiting or refusing if busy, consistent with `start_update_owned`) BEFORE calling `self.register(...)`, validate the row
matches the frozen plan under that lock, and pass `Some(host_write)` into `launch_claimed_run`. This uses the same lock
order already established (`add_confirmations` -> `host_provision_lock` -> DB/reconcile -> `memory`) and ensures neither
`retarget` nor `set_tailscale` can interleave between `register()` and run start, nor can a refused confirmation mutate
`remote_farhelm`/`remote_state_dir`.

Reviewer metadata (g32_remove_host_signin_lock_helm/f1): suggested bucket: high (A rare concurrency failure, but when
hit it durably corrupts the retargeted/mode-changed host's dial coordinates with un-installed paths from a refused
setup, breaking connection to the host with no UI control to clear `remote_farhelm`/`remote_state_dir` short of removing
and re-adding or re-running setup. Does not lose user data, credentials, or processes, so not `highest`.); confidence:
confirmed (established directly from source analysis of `service.rs`, `store.rs`, `hosts.rs`, and the test in
`provisioning.rs`.); possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g32_remove_host_signin_lock_helm/f1`.

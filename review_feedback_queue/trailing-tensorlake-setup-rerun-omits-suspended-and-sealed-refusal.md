# Tensorlake setup rerun omits suspended refusal and probe-time sealed refusal

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

Rerunning setup on a suspended Tensorlake host is not refused up front with a resume-first message; instead, Farhelm
offers the setup confirmation, accepts it, temporarily replaces the host's Suspended status with a setup state, queries
Tensorlake, and then fails the setup run. Similarly, probing a setup rerun on a sealed Tensorlake host awaiting deletion
retry still offers the confirmation dialog before refusing only when confirmed.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/tensorlake_add.rs:747`.

`SPEC_impl.md:6900-6902` requires: "Provisioning does not go through the actor's gate, so it checks the same state
itself. Update and setting a host up again are refused for a host the manager shows suspended, before anything is probed
and again before a confirmed run's preflight probe, with a resume-first message, and the panel offers neither while the
host is suspended." In `crates/farhelm-helm/src/provisioning/http.rs:192-199`, `POST /api/hosts/probe` routes
`ProbeDestination::TensorlakeHost { host }` directly to `ProvisioningService::probe_tensorlake` ->
`probe_tensorlake_owned` (`crates/farhelm-helm/src/provisioning/tensorlake_add.rs:747-781`), bypassing `service.rs`'s
`suspended_refusal` check (`service.rs:948`). Neither `probe_tensorlake_owned` (`tensorlake_add.rs:747-781`) nor
`start_tensorlake_add` (`tensorlake_add.rs:941-1002`) calls `self.suspended_refusal(&row)`. As a result, probing a
suspended Tensorlake host succeeds and mints a confirmation plan (even calling `tensorlake.ensure_host_key()` at line
813), and confirming it enters `start_tensorlake_add`, acquires
`hold_connection(host, HostState::Unreachable { last_error: "Farhelm is setting up this Tensorlake sandbox" })` at line
993 (replacing `HostState::Suspended`), calls `get_sandbox` in `tensorlake_steps` (`tensorlake_add.rs:1224`) and
`create_recorded` (`tensorlake/hosts.rs:409, 487`) before failing at `hosts.rs:491`, leaves a failed setup run, and
calls `release_hold(host, true)` (`tensorlake_add.rs:1061`). Additionally, `probe_tensorlake_owned` omits the
`self.manager.is_sealed(host)` check that `start_tensorlake_add` enforces at `tensorlake_add.rs:984`, so probing a
sealed Tensorlake host still returns a confirmation plan before failing on Confirm. Fix direction: In
`probe_tensorlake_owned` (`ProbeDestination::TensorlakeHost` arm, after loading `row`) and in `start_tensorlake_add`
(for `plan.host = Some(host)` under `rerun_write`, alongside the `is_sealed` check), check
`self.suspended_refusal(&row)` and refuse with the resume-first message; also check `self.manager.is_sealed(host)` in
`probe_tensorlake_owned` before minting a plan or calling `ensure_host_key`.

Reviewer metadata (g07-tensorlake-runtime/f1): suggested bucket: other (could not be in highest: does not create a
duplicate sandbox or lose user data/credentials; produces an improper confirmation dialog, overwrites Suspended with a
failed setup run, and triggers an unnecessary active reconnect ladder); confidence: confirmed; possible cover: none
(distinct from g08/l3 and g08/f3, which cover ordinary SSH-destination setup and Update in provisioning/service.rs
rather than the dedicated TensorlakeHost rerun path in tensorlake_add.rs).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g07-tensorlake-runtime/f1`.

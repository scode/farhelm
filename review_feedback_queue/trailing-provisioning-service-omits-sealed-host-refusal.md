# Ordinary SSH-destination setup and Update paths do not refuse sealed environment hosts

Reviewed commit: dcebd0aa9beb70b8b29ce309a2df600a05dc64f3

## TLDR

While a sealed sandbox or container is waiting for a failed deletion to be retried, Update or SSH-targeted setup
requests are not refused and still probe or install into the machine being deleted.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/service.rs:3081`.

Commit `dcebd0aa` strengthened `setup_of_a_sealed_sandbox_is_refused_before_it_takes_the_host`
(`crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:2196-2248`), which guards `start_tensorlake_add`
(`crates/farhelm-helm/src/provisioning/tensorlake_add.rs:984`, the ONLY `is_sealed` check in
`crates/farhelm-helm/src/provisioning/`). Commit `1512c2e9` clarified
(`crates/farhelm-helm/src/agent_requests.rs:18396-18400`) that after a failed destruction, the connection hold is
released and the actor may reconnect while the seal record keeps `HostState::Deleting { failure: Some(_) }` visible and
blocks session routing. However, `ProvisioningService::suspended_refusal`
(`crates/farhelm-helm/src/provisioning/service.rs:3081-3102`) matches only `Stopped`, `Suspended`, and
`TensorlakeChanging`, returning `None` for `HostState::Deleting`, and neither `probe_owned` (`service.rs:948`),
`claim_register_and_start` (`service.rs:1532`), `plan_update_owned` (`service.rs:1680`), nor `revalidate`
(`service.rs:2770, 2905`) checks `self.manager.is_sealed(host)` or `HostState::Deleting`. While automatic updates skip
`Deleting` (not `Connected`), a direct `POST /api/hosts/{id}/update` (for either a sealed Tensorlake sandbox or a sealed
Podman container) or `POST /api/hosts/probe` with `ProbeDestination::Ssh` targeting a sealed Tensorlake host's
destination is admitted and probes/installs into an environment being deleted. Fix direction: refuse
`HostState::Deleting` (or `self.manager.is_sealed(row.id)`) in `suspended_refusal` (or a shared non-provisionable-state
check used by `probe_owned`, `claim_register_and_start`, `plan_update_owned`, and `revalidate`).

Reviewer metadata (g08-tensorlake-specs-and-tests/f3): suggested bucket: other (cannot be highest: cannot unseal the
supervisor or lose user data/credentials; wastes provisioning/SSH probes and can mutate an environment already marked
for deletion); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g08-tensorlake-specs-and-tests/f3`.

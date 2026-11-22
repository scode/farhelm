# Probe Tailscale-mode mismatch check runs before new_entry and target validation

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

Adding a destination that is already registered with the opposite Tailscale setting via the probe API (or probing a
managed sandbox/container destination with Tailscale enabled) returns a misleading message asking to change Tailscale
mode in Host settings instead of naming the existing host entry or refusing the invalid target.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/service.rs:959-971`.

In `crates/farhelm-helm/src/provisioning/service.rs:959-971`, `ProvisioningService::probe_owned` checks
`if let (Some(row), Some(requested_mode)) = (&stored, requested_mode) && row.tailscale != requested_mode` and returns
`ProvisioningRequestError::Refused("Change the Tailscale mode for this host in Host settings, then probe it again.")`
BEFORE `self.target(&request, access)?` (`service.rs:993`) and BEFORE
`self.store.check_new_entry(destination, entry).await?` (`service.rs:1004`). Two issues result: (1) For an Add-host
request (`request.new_entry.is_some()`), `SPEC.md:308-311` requires that adding a destination that is already a host
entry be refused naming that entry (`HostStoreError::AlreadyAdded`), whereas sending `tailscale: Some(true)` against an
existing non-Tailscale host (or `Some(false)` against an existing Tailscale host) returns the mode-change refusal
without naming the existing entry/alias (note the GUI's `add.rs:457-475` pre-checks against its cached host list, so
this hits API callers or a GUI racing a newly added host). (2) Because `stored` matches ANY row by destination
(`service.rs:891-896`), a `HostKind::Tensorlake` row or a `HostKind::Podman` synthetic `podman:...` row (both of which
have `row.tailscale == false`) probed with `tailscale: Some(true)` also returns "Change the Tailscale mode for this host
in Host settings, then probe it again.", even though managed environment hosts have no Tailscale mode setting (and
`podman:...` is rejected as an unusable SSH destination by `self.target`). Fix: guard the mode-mismatch check with
`row.kind == HostKind::Ssh` and run `check_new_entry` (and `destination_is_usable`) before it.

Reviewer metadata (g45-tailscale-provisioning-podman/f6): suggested bucket: other (not highest: wrong refusal message
only, no security or data-loss impact); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g45-tailscale-provisioning-podman/f6`.

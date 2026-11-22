# Store and host routes discard Tailscale destination format guidance

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

Enabling Tailscale mode on an existing SSH host whose address uses a URI, port, bracketed IPv6 literal, or SSH config
alias with underscores (or editing a Tailscale host to such an address, or registering it via the host API) fails with
"is not a usable ssh destination" instead of explaining that Tailscale mode requires a Tailscale node name, DNS name, or
IP with an optional login.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/store.rs:7291-7296`.

`crate::ssh::tailscale::destination_host` (`crates/farhelm-helm/src/ssh/tailscale.rs:268-294`) returns an actionable
user-facing error: `"Enter a Tailscale node name or IP, optionally preceded by an SSH login and @."`. Probe requests in
`crates/farhelm-helm/src/provisioning/service.rs:821-825` preserve that message verbatim, but every store validation
site (`crates/farhelm-helm/src/store.rs:7291-7296` in `add_ssh_host_with_mode`, `7671-7676` in
`register_probed_ssh_host_with_mode`, `7917-7919` in `ensure_ssh_hosts`, `8061-8065` in `change_ssh_destination`,
`8170-8172` in `set_ssh_tailscale`) as well as `crates/farhelm-helm/src/hosts.rs:998-1003` (`set_tailscale`) discard it
and construct `HostStoreError::InvalidDestination(destination)`, whose `Display` (`store.rs:2057`) is
`"{0:?} is not a usable ssh destination"`. For a user toggling Tailscale host on an already-working SSH entry (e.g.
`ssh://user@box:22` or `my_box`), telling them their address is "not a usable ssh destination" is factually misleading
and hides what format Tailscale mode expects (SPEC.md:292-295). Fix direction: Add a dedicated
`HostStoreError::InvalidTailscaleDestination` variant (mapped to `ErrorKind::InvalidRequest` in `lib.rs:1625`) carrying
`destination_host`'s explanation, or preserve the error text from `destination_host` on an invalid-request variant.

Reviewer metadata (g43-tailscale-store-and-access/f1): suggested bucket: other (not highest — diagnostic/UX clarity
only); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`, `crates/farhelm-helm/src/hosts.rs:998-1003`
(g44-tailscale-hosts-transport/f8):

In `crates/farhelm-helm/src/hosts.rs:998-1003`, `set_tailscale` checks:

```rust
if spec.tailscale
    && let Some(destination) = &old.destination
    && crate::ssh::tailscale::destination_host(destination).is_err()
{
    return Err(HostStoreError::InvalidDestination(destination.clone()).into());
}
```

`crate::ssh::tailscale::destination_host` (`crates/farhelm-helm/src/ssh/tailscale.rs:268-294`) returns an actionable
error: `"Enter a Tailscale node name or IP, optionally preceded by an SSH login and @."` However, `set_tailscale`
discards that error and substitutes `HostStoreError::InvalidDestination(destination.clone())`, whose `Display`
(`crates/farhelm-helm/src/store.rs:2057`) is `"{0:?} is not a usable ssh destination"` (mapped to HTTP 400 in
`crates/farhelm-helm/src/lib.rs:1625`). For an already-registered, working ordinary SSH host (e.g., `ssh://user@box`,
`user@box:2222`, or an SSH alias like `dev_box`), telling the user their destination is not a usable SSH destination
contradicts reality and hides what they need to change (SPEC.md "Topology", lines 292-295, promises actionable errors
for Tailscale host verification/configuration). Note that `store.rs:8170-8172` (`set_ssh_tailscale`) has the same
wrapping (covered by `g43/f1`). Fix direction: Add a dedicated `HostStoreError` variant (e.g.
`InvalidTailscaleDestination` mapped to `ErrorKind::InvalidRequest` in `lib.rs:1625`) carrying `destination_host`'s
actionable guidance, and use it in both `hosts.rs:1002` and `store.rs`.

Reviewer metadata (g44-tailscale-hosts-transport/f8): suggested bucket: other (could NOT be in highest: misleading 400
error wording with no security or data-loss impact).; confidence: confirmed.; possible cover: overlaps with g43/f1
(which covers the matching store.rs call sites and noted hosts.rs:998-1003)..

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g43-tailscale-store-and-access/f1`,
`g44-tailscale-hosts-transport/f8`.

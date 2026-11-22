# ensure-hosts skips Tailscale destination validation for already-registered hosts and drops entry context

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

An invalid Tailscale destination in an ensure-hosts startup file is silently accepted with no error or warning when that
destination is already registered, only to fail startup later on a fresh database or after removing the host; when it
does fail for a new entry, the error does not name the entry index or explain Tailscale's address format.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/store.rs:7916-7919`.

SPEC_impl.md:5665-5667 requires all-or-nothing startup validation for `--ensure-hosts`: "a malformed file, an unusable
destination, or a destination listed twice fails startup with the entry named and nothing written." In
`HelmStore::ensure_ssh_hosts` (`crates/farhelm-helm/src/store.rs:7850-7874`), the up-front validation loop checks
`destination_is_usable(&entry.destination)` and reports `entry {index}`. However, the new Tailscale destination check
(`entry.tailscale && crate::ssh::tailscale::destination_host(&entry.destination).is_err()`) was placed inside
`if !already_registered` at `crates/farhelm-helm/src/store.rs:7916-7919` alongside `require_usable_remote_paths`. Unlike
`require_usable_remote_paths` (which deliberately stayed inside `if !already_registered` so pre-existing ensure files
containing `~/...` would not break upgrades, and paired that leniency with `hosts_needing_correction` /
`warn_hosts_needing_correction` in `crates/farhelm-helm/src/ensure.rs:150-212`), `tailscale` is a newly introduced field
with no legacy files to preserve, and `ensure.rs` emits no warning for it. As a result:

1. Adding `tailscale: true` to an already-registered entry whose destination is invalid for Tailscale (such as
   `ssh://user@box:22` or `box:22`) passes startup silently, then fails later if the host is removed or the file is
   reused on a fresh database.
2. When a new entry fails at `store.rs:7918`, it returns `HostStoreError::InvalidDestination(entry.destination)`
   (`"{dest:?} is not a usable ssh destination"`), dropping both the `entry {index}` locator and `destination_host`'s
   Tailscale format guidance. Fix direction: Validate
   `entry.tailscale => crate::ssh::tailscale::destination_host(&entry.destination)` in the up-front loop at
   `crates/farhelm-helm/src/store.rs:7850-7874`, naming `entry {index}` and including `destination_host`'s error
   message.

Reviewer metadata (g43-tailscale-store-and-access/l3): suggested bucket: other (not highest — no security or data-loss
impact); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g43-tailscale-store-and-access/l3`.

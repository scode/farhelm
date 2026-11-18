# Podman places can bypass the helm’s duplicate-host check

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Farhelm can treat two SSH entries for the same machine and account as usable container places even while its ordinary
host connection refuses the duplicate.

## Details

Bucket: **high**. GUI migration final whole-tree review, R9-7 follow-up.

Source references: `crates/farhelm-helm/src/environment/podman_probe.rs:393, 476–520`,
`crates/farhelm-helm/src/manager.rs:4643–4688`, `crates/gui/farhelm-gui/src/settings/containers.rs:589–628`.

The place probe caches a successful command chain for a registered route by host entry and does not check the installed
supervisor's identity or compare equivalent compute and storage endpoints. The ordinary supervisor handshake refuses a
duplicate identity and identifies the owning entry. A LAN address and a tailnet address can therefore give Podman two
places for one account.

Opus verification R9-7 confirmed the cross-place orphan Delete as critical and suggested this separate follow-up. #4083
now hides the tracked container at another place and refuses its orphan Delete across all recorded places. The probe
still lacks twin and identity refusal. High reflects confusing or unsafe duplicate place admission. A remaining
wrong-target destruction or credential route would need its own proof to move this follow-up to highest.

Apply the ordinary supervisor's identity refusal when available and compare an appropriate stable machine/account
identity for usable Podman places. Preserve the supported route for a place that can run Podman independently of its
supervisor. Verify that two spellings for the same endpoint do not become independent places. SPEC.md lines 313–317
supplies the ordinary host rule. The separate SSH-retarget false-absence finding covers a changed endpoint rather than a
twin.

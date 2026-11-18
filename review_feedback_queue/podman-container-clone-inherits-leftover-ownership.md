# A Podman clone can appear as a Farhelm-owned leftover

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A container you clone and use independently can show up in Settings as a Farhelm leftover. Deleting it from that list
permanently removes the clone and its work.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R9-9.

Source references: `crates/farhelm-helm/src/environment/podman.rs:279–286, 729–770, 782–850, 1430–1445` at the reviewed
commit (affected main lines near `crates/farhelm-helm/src/environment/podman.rs:306–313, 754–789, 801–865, 1445–1460`),
`crates/gui/farhelm-gui/src/settings/containers.rs:255–300`.

Podman carries configuration labels into a clone. An independent copy with a new canonical ID and new name can therefore
retain this helm's Farhelm role and recipe labels. The orphan path treats those labels as ownership, sees no recorded
host, recipe, or active job, and removes the selected clone. The name need not be in a Farhelm-shaped namespace. The GUI
confirms deletion of the actual named clone, including all work inside it.

Sol rated this source-confirmed and critical. Opus verification R9-9 confirmed it and rated it noncritical given the
inherited labels and explicit confirmation of the named clone. Highest reflects the established loss of independent
container work after the list claims leftover ownership. SPEC_impl.md lines 5833–5835 and 5879–5881 define
label-filtered discovery and rechecks. Whether this authorizes treating deliberately cloned labels as ownership is
unclear.

Distinguish canonical IDs the helm created from copied labels. Flag other labelled IDs as external or require
authoritative creation provenance, while preserving recovery of truly interrupted creates.

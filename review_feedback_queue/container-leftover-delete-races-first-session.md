# Leftover cleanup can delete a container whose first session just started

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A container can appear as a leftover while Farhelm is still launching its first session. If you open Delete then and
confirm after the launch finishes, it can delete the new session and its work.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R9-8.

Source references: `crates/farhelm-helm/src/environment/podman.rs:829–848` at the reviewed commit (affected main lines
near `crates/farhelm-helm/src/environment/podman.rs:843–863`),
`crates/gui/farhelm-gui/src/settings/containers.rs:255–300, 664–675`,
`crates/farhelm-helm/src/provisioning/tensorlake_add.rs:1131`, `crates/farhelm-helm/src/hosts.rs:1306–1372`.

A row-backed container appears in the leftovers list while its cached sessions are empty. The row can be in the middle
of its first launch. The question freezes that leftover row and warns that deleting the named container removes all of
its sessions, processes and files. Once the first launch completes, the question can still send an ordinary whole-host
Delete carrying only the host row ID. The provisioning lock delays removal but does not recheck the leftover condition
after it acquires the lock.

Sol rated this source-confirmed and critical. Opus verification R9-8 confirmed the route and rated it noncritical
because the user picks the named container during their own launch and approves its explicit file-and-session warning.
Highest reflects the established loss of the new session and checkout. SPEC_impl.md lines 5833–5835 explicitly directs
row-backed leftovers through ordinary confirmed whole-host Delete with the lock. Whether that procedure allows the
leftover to become a live environment during confirmation is unclear. The cross-place ownership fix in #4083 did not add
a no-sessions check at dispatch.

Exclude actively provisioning rows from leftover discovery. Bind the Delete to the listed ID and no-session condition
and revalidate under the provisioning lock. Test confirmation issued before the first session is published and answered
after it is present.

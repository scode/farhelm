# Retry can continue a failed sandbox deletion when its menu still showed a connection retry

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A Retry still shown as reconnecting can continue a sealed sandbox deletion if the failed deletion state arrives before
the menu updates.

## Details

The menu gives both commands the same id and focus handle and sends only the host id
(crates/gui/farhelm-gui/src/hosts/menu.rs:310–339). `remedies::retry` rereads the host and chooses `/retry-delete` when
it now has a failed deletion (crates/gui/farhelm-gui/src/hosts/remedies.rs:102–135, :177–191). GPUI can draw a dirty
frame before delivering Enter without showing that frame, the reason the adjacent Adopt action freezes its target
(menu.rs:260–281; SPEC_impl.md:973–991). A host already visible with ordinary Retry can subsequently report a seal on a
listing, including recovery after a lost reply (SPEC.md:1597–1618. Crates/farhelm-helm/src/manager.rs:5715–5723). The
finisher runs, and failure becomes a pushed status (crates/farhelm-helm/src/sessions/sandbox.rs:294–323, :392–410.
Manager.rs:2840–2883). The delete retry route checks that seal and takes the provisioning lock, then finishes
environment destruction without another prompt (sandbox.rs:326–378). SPEC.md:5500–5505 binds the operation to the
presented frame. Mitigation: capture whether the presented action was reconnect or retry-delete, submit that distinct
verb, and use live state only to refuse when its preconditions have gone stale. Apply the same binding to other controls
that call the shared retry helper if they also present either operation.

Confidence: Unverified after investigation. The changed failed-seal publication and the live Retry dispatch are
confirmed. The remaining premise is that the previous menu stays actionable across that particular transition.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `terminal/l5`, `hostUpdates/l2`.

# “Always allow” can grant permission after its host connection has been replaced

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An old “Always allow” answer could grant a replacement host lasting authority.

## Details

F6 — **possible** — `crates/farhelm-helm/src/approvals.rs:534` — “Always allow” can grant permission after its host
connection has been replaced

The approval handler checks the requesting connection before waiting for the host write lock, then persists permission
without checking that connection again. Its installation-identity condition accepts a missing identity matching another
missing identity. With an identity-less supervisor and concurrent retargeting, an old card could therefore grant the
replacement lasting permission to act across the fleet. Refusing the original action does not undo that grant. Recheck
the originating connection under the lock before writing permission.

## Evidence and triage context

- crates/farhelm-helm/src/approvals.rs:528 checks origin liveness before :534 awaits the host write lock; :537 writes
  without another liveness check.
- crates/farhelm-helm/src/store.rs:4983 uses host_identity IS ?2, permitting NULL-to-NULL matches.
- crates/farhelm-helm/src/hosts.rs:778 holds that write lock across destination update and registry synchronization.
- crates/farhelm-helm/src/manager.rs:1599 withdraws the old client and :1613 changes its incarnation during retarget.
- crates/farhelm-helm/src/manager.rs:3664 admits a missing peer identity when the row also lacks one, returning
  Connected at :3717.
- crates/farhelm-helm/src/store.rs:5196 records first identity without clearing commands_without_asking;
  crates/farhelm-helm/src/approvals.rs:301 subsequently bypasses approval when that setting is true.
- approvals.rs:528 checks origin before waiting at :534, then writes at :537 without rechecking. store.rs:4983-4985
  permits NULL IS NULL. hosts.rs:778-808 retargets under the same lock; manager.rs:1599-1617 retires the old connection.
  manager.rs:3664-3721 admits identity-less peers when no identity is recorded; store.rs:5196 records first identity
  without clearing permission. SPEC.md:580-586 covers default destination selection only, and TRIAGE_OUTCOMES.md:4139
  covers production update planning, so neither matches this durable authority grant.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:580-586 accepts identity-less ambiguity only for default destination selection. TRIAGE_OUTCOMES.md:4139
  concerns update planning with current production supervisors, not an untrusted peer granting durable authority.
  SPEC_impl.md:4035-4042 accepts request expiry after a setting write, but requires the card's connection to remain live
  and does not accept granting a replacement.

Caveats:

- Requires an identity-less nonstandard or compromised supervisor and a concurrent retarget. Ordinary changed non-null
  identities are protected. No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_02_cor:p1:F1`,
`helm_state_provisioning_02_sec:p1:C4`.

- `helm_state_provisioning_02_cor:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
- `helm_state_provisioning_02_sec:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket
  as filed: not separately tagged in candidate list.

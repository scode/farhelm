# Retargeting a never-contacted host preserves exemptions intended for its previous destination

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Retargeting a never-contacted host carries its command exemption to the new destination.

## Details

F10 — **definite** — `crates/farhelm-helm/src/store.rs:4848` — Retargeting a never-contacted host preserves exemptions
intended for its previous destination

An identity-less registered host can already have permission to issue commands without asking. Changing its destination
preserves that permission, and first contact records the new installation identity without clearing it. The newly
contacted machine consequently inherits authority to act across the fleet without a permission choice bound to it.
Invalidate command authority when retargeting such a row, or bind the grant to the selected destination and
installation. The previously covered YOLO behavior is outside this remainder.

## Evidence and triage context

- crates/farhelm-helm/src/store.rs:4934–4952 permits enabling command authority on any existing row without requiring
  host_identity.
- crates/farhelm-helm/src/hosts.rs:777–782 calls the destination writer and reconciles the connection; it adds no
  permission reset.
- crates/farhelm-helm/src/store.rs:4848 updates only destination.
- crates/farhelm-helm/src/store.rs:5180–5200 accepts first contact for the new destination and updates only
  host_identity.
- crates/farhelm-helm/src/approvals.rs:301–302 then bypasses approval; agent_requests.rs:357–368 shows a concrete
  acting-command caller.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- store.rs:5232–5241 may express an intention to apply the older decision to both permissions. No matching explicit
  fleet-command acceptance was found in SPEC.md, SPEC_impl.md, TODO.md Planned, BUGS.md, the queue or the ledger.

Caveats:

- Retain the command-authority remainder; do not reopen the covered YOLO portion merely by retaining this entry.
- Requires an identity-less registered row with the exemption enabled before retargeting.
- No race is necessary.
- No runtime reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_10_cor:p1:F3`.

- `helm_state_provisioning_10_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest.

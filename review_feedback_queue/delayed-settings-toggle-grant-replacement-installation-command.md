# A delayed settings toggle can grant a replacement installation command authority

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A delayed checkbox action can enable fleet commands on a replacement installation.

## Details

F9 — **definite** — `crates/farhelm-helm/src/store.rs:4951` — A delayed settings toggle can grant a replacement
installation command authority

The settings checkbox sends only the host-row identifier and its new value. If that request arrives after adoption has
reset permissions for a replacement installation, the row-only update enables command authority on the replacement. That
authority includes stopping sessions on other hosts without another approval. Carry the displayed installation identity
into the request and compare it atomically when granting permission. The earlier decision about YOLO confirmation does
not settle acceptance of this broader authority.

## Evidence and triage context

- crates/farhelm-ui/src/hosts.rs:1335–1341 and api.rs:3455–3464 send only registry ID and checkbox value.
- crates/farhelm-helm/src/hosts.rs:547–548,584–593 supplies no expected identity; the host write lock serializes writes
  but cannot bind them to the installation originally displayed.
- crates/farhelm-helm/src/store.rs:5310–5312 resets commands_without_asking during adoption; line 4951 can subsequently
  set it by ID alone.
- crates/farhelm-helm/src/approvals.rs:297–302 immediately allows requests when that flag is true.
- crates/farhelm-helm/src/agent_requests.rs:357–368 uses that approval result to stop a routed target session; lines
  1485–1499 only recheck the requesting connection's liveness.
- crates/farhelm-helm/src/store.rs:4983–4985 demonstrates the identity-conditional write already used for approval
  cards.
- crates/farhelm-ui/src/hosts/settings_dialog.rs:358–359 emits ID/value events; api.rs:3455–3464 sends no expected
  identity.
- crates/farhelm-helm/src/hosts.rs:584–593 serializes and performs the unguarded setter.
- crates/farhelm-helm/src/store.rs:5310–5312 resets the flag during adoption, but line 4951 subsequently accepts any
  ID-matching write.
- crates/farhelm-helm/src/approvals.rs:301–302 treats the flag as approval; agent_requests.rs:365–368 consequently
  permits a target-session stop.
- crates/farhelm-helm/src/store.rs:4983–4985 guards the separate approval-card setter with host_identity.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:4923–4943 discards stale YOLO toggles. store.rs:5232–5241 discusses both settings before citing
  that decision. This creates possible intended coverage, but the ledger does not match the fleet-command consequence
  and scope. SPEC.md:174–178 and SPEC_impl.md:4035–4039 treat the permission as installation-specific.
- TRIAGE_OUTCOMES.md:4923–4943 explicitly discards stale YOLO settings writes. store.rs:5232–5241 is broader, but cites
  that narrower decision. Its applicability to fleet-command authority remains ambiguous.

Caveats:

- Requires a stale or delayed settings request executing after adoption.
- The code behavior is definite; whether the maintainer intended the older YOLO disposition to cover this stronger
  permission remains unresolved.
- Approval-card writes use the guarded setter and are not this finding.
- No runtime reproduction was performed.
- Duplicate claim retained as a separate input entry.
- Requires a delayed request or concurrent client adoption.
- Behavior is definite; intended acceptance remains ambiguous.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_10_cor:p1:F1`,
`helm_state_provisioning_10_sec:p1:F1`.

- `helm_state_provisioning_10_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `helm_state_provisioning_10_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.

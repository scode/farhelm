# A stale host-settings request can grant fleet authority to a replacement installation

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A stale settings request can grant a replacement host permission to act across the fleet.

## Details

F7 — **definite** — `crates/farhelm-helm/src/hosts.rs:590`; `crates/farhelm-helm/src/hosts.rs:592` — A stale
host-settings request can grant fleet authority to a replacement installation

The command-permission setting is submitted with the persistent host-row identifier, without the installation identity
displayed to the user. Adoption clears the replacement's permission, but a delayed settings request can restore it
afterward because it still matches that row. The replacement can then issue acting commands without approval. Include
the displayed installation identity in permission grants and compare it atomically when writing the setting. This is
separate from the identity-checked approval-card path.

## Evidence and triage context

- crates/farhelm-ui/src/api.rs:3455 accepts host ID and boolean; :3464 sends no expected installation identity.
- crates/farhelm-helm/src/hosts.rs:547 contains only the boolean; :589 takes the write lock; :592 forwards a row-only
  setter.
- crates/farhelm-helm/src/store.rs:4951 updates commands_without_asking using only WHERE id = ?1.
- crates/farhelm-helm/src/store.rs:5310 atomically clears the setting during adoption, but does not prevent a later
  stale setter from restoring it.
- crates/farhelm-helm/src/approvals.rs:301 treats the restored setting as authority to bypass approval.
- crates/farhelm-ui/src/hosts/settings_dialog.rs:545 emits only (id, checked); crates/farhelm-ui/src/hosts.rs:1335–1340
  forwards those values.
- crates/farhelm-ui/src/api.rs:3455–3464 sends the row ID and commands_without_asking boolean, without an expected
  install identity.
- crates/farhelm-helm/src/hosts.rs:584–593 takes the host write lock but supplies no identity precondition.
- crates/farhelm-helm/src/store.rs:4934–4955 selects and updates by id alone. Adoption resets commands_without_asking
  while replacing host_identity at store.rs:5305–5313; a subsequent stale setter therefore enables it on the
  replacement.
- crates/farhelm-helm/src/approvals.rs:297–302 bypasses approval when this setting is true. agent_requests.rs:1491–1498
  checks connection liveness afterward, which does not reject a new request from the replacement's current connection.
- crates/farhelm-helm/src/approvals.rs:534–538 and store.rs:4983–4985 protect the separate approval-card path with an
  atomic identity comparison.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:4923-4943 explicitly discards stale YOLO toggles, whose scope is launches on that host. This setter
  grants that host authority over other hosts and templates. SPEC.md:2235 makes multiple GUIs best effort but still asks
  for easy fixes; it does not unambiguously accept this security consequence.
- TRIAGE_OUTCOMES.md:4923–4945 discards stale YOLO-setting variants. Its consequence is bypassing YOLO launch
  confirmation, not granting a replacement install authority to change sessions and templates across the fleet.

Caveats:

- Requires a request or dialog based on the predecessor crossing adoption, commonly involving another GUI. No runtime
  reproduction. Distinct from the Always allow handler, which uses a different setter and has a different race.
- Requires a stale or delayed authenticated settings request crossing adoption; a remote supervisor cannot invoke this
  GUI endpoint directly.
- No runtime reproduction was performed.
- The identity-checked approval-card path is unaffected.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_03_cor:p1:F1`,
`helm_state_provisioning_03_sec:p1:F1`.

- `helm_state_provisioning_03_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `helm_state_provisioning_03_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.

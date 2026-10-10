# Fixture startup can leave a real systemd service behind

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed fixture startup can leave a real user service running.

## Details

F122 — **definite** — `crates/farhelm-helm/src/provisioning.rs:9238` — Fixture startup can leave a real systemd service
behind

The fixture enables its sleep service and creates a runtime unit link before constructing the cleanup guard. A setup
failure in that interval returns without stopping the service or removing the link, leaving changes in the shared user
manager. Construct the cleanup guard before enabling the service and retain it through every fallible setup step.

## Evidence and triage context

- crates/farhelm-helm/src/provisioning.rs:9238 enables and starts the runtime unit.
- crates/farhelm-helm/src/provisioning.rs:9254 and :9255 can panic while spawning or checking tmux.
- crates/farhelm-helm/src/provisioning.rs:9261 constructs UnitGuard only afterwards; its cleanup at :7942 and Drop at
  :8027 disable and stop the unit.
- crates/farhelm-helm/src/provisioning.rs:9238–9247 activates a real runtime unit before tmux setup.
- crates/farhelm-helm/src/provisioning.rs:9254–9258 can panic before UnitGuard construction at :9261.
- crates/farhelm-helm/src/provisioning.rs:8027–8037 shows the cleanup that consequently never runs.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires a usable user manager and failure after service activation. Fixture resources are nonce-scoped; unrelated
  user-process loss is not established. Inspection only.
- Nonce-scoped fixture resources only. The sleep process naturally ends after 300 seconds, but that does not remove the
  leaked runtime link.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_06_cor:p1:F1`,
`helm_state_provisioning_06_sec:p1:F4`.

- `helm_state_provisioning_06_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `helm_state_provisioning_06_sec:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.

# Helm cannot start when Farhelm’s executable path contains `$`

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A dollar in the executable path prevents the helm service from starting.

## Details

F93 — **definite** — `crates/farhelm-helm/src/units.rs:178` — Helm cannot start when Farhelm’s executable path contains
`$`

The helm service renderer independently doubles dollars in the executable position. Systemd therefore looks for a
different pathname and the service cannot launch, leaving the browser UI unavailable. This path remains reachable when
setup omits the supervisor. Use executable-specific quoting here while retaining dollar escaping for ordinary service
arguments.

## Evidence and triage context

- crates/farhelm-helm/src/units.rs:175–180 passes the helm executable through systemd_arg, whose line 382 doubles
  dollars.
- crates/farhelm-helm/units/farhelm-helm.service.in:5 places the result in the executable position.
- crates/farhelm/src/setup.rs:605–606 resolves the executable; lines 671–682 always add the helm unit, outside the
  no_supervisor condition at line 642.
- The inspected retained systemd check 04ff10c6-efba-430e-b75a-8abc60a57a37 confirms executable lookup uses the doubled
  filename.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Shares the quoting helper with the supervisor finding but has a separate renderer anchor.
- Requires a literal dollar in the resolved executable path.
- No new runtime verification was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_11_cor:p1:F2`.

- `helm_state_provisioning_11_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: high.

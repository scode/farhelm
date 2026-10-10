# Supervisor cannot start when Farhelm’s executable path contains `$`

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A dollar in the executable path makes the supervisor service unusable.

## Details

F92 — **definite** — `crates/farhelm-helm/src/units.rs:152` — Supervisor cannot start when Farhelm’s executable path
contains `$`

Supervisor setup and provisioning apply argument-style dollar escaping to the executable pathname. Systemd opens that
pathname without the corresponding argument expansion, so it looks for a doubled-dollar filename rather than the
selected executable. Repeating setup regenerates the broken unit and the supervisor cannot start. Use
executable-specific quoting that preserves literal dollars, and align executable read-back with systemd's semantics.

## Evidence and triage context

- crates/farhelm-helm/src/units.rs:152 passes the executable to systemd_arg; lines 375–383 double every dollar.
- crates/farhelm-helm/units/farhelm-supervisor.service.in:6 places that value in the executable position of ExecStart.
- crates/farhelm/src/setup.rs:605–606 canonicalizes the actual executable and lines 664–668 pass it directly to the
  supervisor renderer.
- crates/farhelm-helm/src/provisioning/plan.rs:558–565 schedules the rendered unit for writing; lines 796–805 delegate
  to the same renderer.
- Inspected dollar_path.py:4–14 and retained run 04ff10c6-efba-430e-b75a-8abc60a57a37: the literal executable passed
  systemd verification; the doubled filename failed with 'not executable: No such file or directory'.
- crates/farhelm-helm/src/units.rs:301–303 also incorrectly collapses doubled dollars when reading the executable back.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires a literal dollar in the resolved executable path and no executable at the doubled-dollar path.
- The retained parser check was inspected, not rerun; no service was started.
- No process-loss consequence is established by this finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_11_cor:p1:F1`.

- `helm_state_provisioning_11_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.

# Interrupted paired writes can still split service state directories

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Interrupted service reconfiguration can persist incompatible state directories.

## Details

F96 — **definite** — `crates/farhelm/src/setup.rs:1146` — Interrupted paired writes can still split service state
directories

The supervisor service definition is published before the helm definition. Terminating the process after the first
publication bypasses rollback, leaving the supervisor on the new state directory and the helm on the old one. A
subsequent reboot can consume this mixed pair and start services that cannot communicate. Make paired reconfiguration
recoverable before either service consumes an incomplete pair, or refuse transitions that cannot meet that contract.

## Evidence and triage context

- setup.rs:661–681 plans supervisor then helm. write_units_together at :1140–1167 publishes them individually; its
  restoration exists only in the returned-error branch. write_unit at :1195–1207 independently persists each file. The
  service templates directly consume their separately published state-directory arguments.
- setup.rs:1140–1167 keeps the previous texts and completed indexes in memory and restores only after write_unit returns
  an error. :1204–1207 publishes one file at a time; :661–681 orders supervisor before helm. Neither service template
  contains paired-publication recovery.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- SPEC.md:417–427 explicitly prohibits leaving services configured against different directories.
  TRIAGE_OUTCOMES.md:3747–3762 records the previous fix for fallible pre-write steps and returned write errors, not
  termination between successful publications. :4191–4202 concerns deliberate --no-supervisor use, a different trigger.
- SPEC.md:417–427 prohibits this result. TRIAGE_OUTCOMES.md:3747–3762 covers the earlier returned-error/pre-write
  failure mechanism, not this crash residual; :4191–4202 covers --no-supervisor only.

Caveats:

- Requires changing a previously installed pair's state directory and interruption between publications.
- Rerunning setup with the intended options repairs the pair.
- No runtime reproduction or established permanent data loss.
- The durable configuration mismatch is outside the rare-diagnostic filter.
- Requires changing an existing pair and interruption between publications. Rerunning setup repairs it; no permanent
  data loss demonstrated. Same location as cli_installation_02_sec:p1:F1.
- No runtime reproduction.
- A repeated setup can repair the pair, but that does not prevent a reboot from consuming it first.
- No permanent data loss established.
- No runtime reproduction or permanent data loss established. Same-location duplicate of cli_installation_02_cor:p1:F3.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_02_cor:p1:F3`,
`cli_installation_02_sec:p1:F1`.

- `cli_installation_02_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: high.
- `cli_installation_02_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.

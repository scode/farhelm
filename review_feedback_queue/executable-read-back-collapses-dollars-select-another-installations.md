# Executable read-back collapses dollars and can select another installation’s service

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Uninstall can mistake a service with literal dollars for another installation's service.

## Details

F11 — **definite** — `crates/farhelm-helm/src/units.rs:301` — Executable read-back collapses dollars and can select
another installation’s service

The executable-path reader turns each literal `$$` pair into one dollar, although systemd preserves those dollars in the
executable pathname. A marked service running `/opt/$$name/farhelm` can thus be classified as belonging to
`/opt/$name/farhelm`. If those paths identify different installations, uninstall can stop and delete the wrong service;
its later check uses the same faulty reader. Preserve literal dollars when reading executable paths, separately from
argument-expansion rules.

## Evidence and triage context

- crates/farhelm-helm/src/units.rs:301–303 converts each literal $$ pair into one dollar.
- Systemd v255 src/core/load-fragment.c:890, :934 and :1048 preserve executable dollars; src/core/exec-invoke.c:4739
  opens that path independently of argv expansion at :5186.
- crates/farhelm/src/setup.rs:498–517 accepts a regular setup-marked unit, parses its executable, canonicalizes the
  erroneous pathname, and compares it against the selected installation.
- crates/farhelm/src/setup.rs:334–335 adds a matching unit to the removal plan. crates/farhelm/src/uninstall.rs:301
  repeats the same parser-based preflight, so the post-confirmation check does not correct the error.
- crates/farhelm/src/uninstall.rs:184 invokes removal; crates/farhelm/src/setup.rs:898 runs disable --now and :914
  deletes the selected unit.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Wrong-installation removal requires distinct dollar spellings that resolve to different installations, a recognized
  Linux installation being removed, and a setup-marked unit.
- The destructive scenario was not executed.
- The hosts panel's parser use only selects refusal wording; this finding concerns standalone uninstall.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_12_cor:p1:F2`.

- `helm_state_provisioning_12_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.

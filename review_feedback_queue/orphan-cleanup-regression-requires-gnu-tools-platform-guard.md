# Orphan-cleanup regression also requires GNU tools without a platform guard

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The orphan-cleanup test fails on macOS before its intended sequence.

## Details

F124 — **definite** — `crates/farhelm-helm/src/provisioning.rs:7468` — Orphan-cleanup regression also requires GNU tools
without a platform guard

The separate orphan-cleanup fixture also invokes incompatible Linux metadata commands without a platform boundary and
requires them to succeed. Native macOS execution stops during prerequisite inspection rather than completing upload,
installation, and orphan cleanup. Apply a Linux-only fixture boundary or explicitly provide compatible tools, so
supported runs either exercise the intended behavior or report the unavailable substrate accurately.

## Evidence and triage context

- crates/farhelm-helm/src/provisioning.rs:7467–7468 has no Linux guard; :7478 creates the installed destination.
- crates/farhelm-helm/src/provisioning.rs:7486 selects ScriptedTransferLauncher, whose :190 executes non-upload commands
  locally.
- crates/farhelm-helm/src/provisioning/backend.rs:1019–1021 performs cleanup then metadata inspection; :748–750 requires
  Linux checksum/stat tools.
- crates/farhelm-helm/src/provisioning.rs:7508 expects upload success and therefore fails on native macOS.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Inspection only; a GNU-tool PATH can conceal the defect. This remains an independently editable test site.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_06_sec:p1:F2`.

- `helm_state_provisioning_06_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.

# Tampering regression fails before reaching its tampering boundary on macOS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The tampering test never reaches byte tampering on native macOS.

## Details

F125 — **definite** — `crates/farhelm-helm/src/provisioning.rs:7537` — Tampering regression fails before reaching its
tampering boundary on macOS

This third fixture independently uses unguarded Linux metadata commands during its prerequisite upload. Native macOS
tools fail there, before the test changes uploaded bytes or checks integrity refusal. Gate the Linux-shell fixture to
its supported platform, or supply compatible tools explicitly so the test reaches its actual tampering boundary.

## Evidence and triage context

- crates/farhelm-helm/src/provisioning.rs:7536–7537 has no Linux guard; :7542 creates the existing destination and :7549
  selects the local-executing launcher.
- crates/farhelm-helm/src/provisioning/backend.rs:1021 reaches :748–750 before uploading.
- crates/farhelm-helm/src/provisioning.rs:7571 unwraps upload success; the tampering operation is later at :7575.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No runtime reproduction. The finding does not establish a production integrity bypass.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_06_sec:p1:F3`.

- `helm_state_provisioning_06_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.

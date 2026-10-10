# Failed-upload regression fails on native macOS tools

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The failed-upload test stops at incompatible macOS metadata tools.

## Details

F123 — **definite** — `crates/farhelm-helm/src/provisioning.rs:7414` — Failed-upload regression fails on native macOS
tools

This test runs Linux-specific metadata commands locally without a platform guard. Native macOS tools reject that
prerequisite inspection, so the test fails before simulating the upload failure it is meant to cover. The verdict
reflects an undeclared substrate dependency rather than the regression. Restrict this Linux-shell fixture to Linux or
explicitly provide and select compatible required tools.

## Evidence and triage context

- crates/farhelm-helm/src/provisioning.rs:7413–7414 has no Linux platform guard; :7419 creates an existing destination.
- crates/farhelm-helm/src/provisioning.rs:182–190 substitutes only the upload command and executes other remote commands
  using local sh.
- crates/farhelm-helm/src/provisioning/backend.rs:1021 inspects that destination before upload; :748–750 invokes
  sha256sum and stat -L -c '%a'.
- crates/farhelm-helm/src/provisioning.rs:7452 requires the later transfer/digest diagnostic, which the metadata failure
  does not produce.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No macOS execution was performed. GNU tools shadowing native tools can mask the failure. This is a test-execution
  defect, not a failure to provision Linux from macOS.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_06_sec:p1:F1`.

- `helm_state_provisioning_06_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

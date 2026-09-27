# The E2E backend gate trusts the directory's ownership and ships in release builds

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A release helm started with the test-only provisioning variable still enables the simulated backend from any marked
directory inside its state directory, without checking who owns that directory or its marker.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Narrowed from the original finding (pre-pr-review-swarm run `20260925-0602-2b597e9-f96c`, `F31 / SEC-E2E-GATE-LEXICAL`).
The gate in `provisioning/e2e.rs` (`E2eProvisioningBackend::new`) now canonicalizes both the configured directory and
the helm state directory before the containment check, so `..` components and symlinks inside the state directory can no
longer point it outside.

What remains are the finding's two further suggestions. The gate does not require the directory and its `ENABLED` marker
to be owned by the current uid and not group- or world-writable, and the seam is still compiled into
`farhelm_release_build` binaries (`provisioning/service.rs` reads `FARHELM_E2E_PROVISIONING_BACKEND_DIR` in every
production helm). Setting the variable already requires control of the helm's own account, which SPEC.md accepts as the
security boundary, so this is defence in depth rather than a standalone exploit.

User-visible consequence: none in normal use; a release helm started with the test-only variable is not refused on
ownership or build grounds.

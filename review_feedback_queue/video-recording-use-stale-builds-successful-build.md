# Video recording can use stale builds after a successful build

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Video capture independently launches fixed paths after building elsewhere.

## Details

F272 — **definite** — `scripts/readme-video.sh:60` — Video recording can use stale builds after a successful build

This helper inherits an external target-directory setting for builds but selects checkout-local outputs when launching
components. A successful refresh can consequently record old or mixed-version behavior rather than the current checkout.
Share resolved output paths between build and launch at this helper, or reject incompatible target overrides.

## Evidence and triage context

- scripts/readme-video.sh:60–67 inherits Cargo's target override but checks checkout-local outputs; :75 invokes
  readme-video.config.ts. That configuration's :55 selects e2e/readme-hero/start-stack.sh, whose :35–43 choose the fixed
  local binaries and bundle.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires stale local outputs and an external build target; no recording was executed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_09_cor:p1:F2`.

- `automation_website_09_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.

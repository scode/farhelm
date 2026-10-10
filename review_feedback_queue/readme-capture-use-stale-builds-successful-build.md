# README capture can use stale builds after a successful build

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

README capture can photograph stale behavior despite a successful rebuild.

## Details

F271 — **definite** — `scripts/readme-screenshot.sh:50` — README capture can use stale builds after a successful build

An external target-directory override sends build output elsewhere, while capture still launches fixed checkout-local
binaries. Successful compilation therefore does not establish that the photographed product represents the current
checkout. Resolve one build-output directory for both compilation and launch, or refuse incompatible overrides before
producing the generated image.

## Evidence and triage context

- scripts/readme-screenshot.sh:50 runs cargo build with the inherited environment; :56 checks only checkout-local
  target/debug/farhelm; :67 invokes the hero configuration. e2e/readme-hero.config.ts:57 selects start-stack.sh, whose
  :35–43 hardcode checkout-local binaries and bundle. scripts/docs-screenshots.sh:75–76 independently recognizes this
  exact mismatch.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires an external target directory and usable stale outputs under the checkout's target directory.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_09_cor:p1:F1`.

- `automation_website_09_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

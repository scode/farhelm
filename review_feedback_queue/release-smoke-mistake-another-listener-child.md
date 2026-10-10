# Release smoke can mistake another listener for its child

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Release smoke could validate another listener instead of its launched helm.

## Details

F312 — **possible** — `.github/workflows/sign-sums.yml:285-317` — Release smoke can mistake another listener for its
child

The port reservation is released before launch, and HTTP checks identify only that port. If another HTML server acquires
it, the helm can fail to bind while the checks pass and the script ignores failed child signaling. The competing
listener was not observed. Establish child readiness and continued process identity, and require product-specific
evidence before declaring artifact validation successful.

## Evidence and triage context

- .github/workflows/sign-sums.yml:285 closes the port reservation; :287-291 launches the child without checking
  readiness or exit; :298 and :304 query only the port; :305 ignores kill failure; :306-317 accepts HTTP 200 plus
  text/html. crates/farhelm-helm/src/lib.rs:1885-1892 propagates bind failure. .github/workflows/release.yml:531-552
  uses this validation before announcement. review_feedback_queue/preview-ready-identity.md:11-26 concerns
  website/scripts/preview.sh, a different independently editable site and consequence, so it is not coverage.
  SPEC.md:1905-1908 accepts browser impersonation on a shared local machine, not a release-validation false pass.
  FILTER.md:41 excludes reported success for a failed operation.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_01_sec:p1:C3`.

- `automation_website_01_sec:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

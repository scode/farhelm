# The “long cwd” test never supplies a long path

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The long-folder test could pass after ellipsis styling is removed.

## Details

F307 — **possible** — `e2e/tests/sidebar.spec.ts:1273–1302` — The “long cwd” test never supplies a long path

Its fixture supplies a short path and checks only DOM text and direction. Those values remain unchanged if the
initial-ellipsis styling is removed, so the test protects direction but not its stated left-truncation contract. Use an
actually overflowing path and an observation that distinguishes the expected tail and ellipsis behavior, retaining the
useful direction checks.

## Evidence and triage context

- e2e/tests/sidebar.spec.ts:1264–1273 states the tail/ellipsis contract; :1279 supplies /tmp; :1287–1302 asserts only
  DOM text and the two computed directions. crates/farhelm-ui/assets/app.css:3427–3429 supplies overflow, ellipsis and
  nowrap separately from direction at :3442. Removing text-overflow: ellipsis would violate the stated ellipsis behavior
  without changing any asserted value. No exact coverage was identified.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_09_sec:p1:C4`.

- `test_infrastructure_09_sec:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

# Retained failed tags become mandatory upgrade-test inputs

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed release tags could make standard upgrade validation impossible.

## Details

F269 — **possible** — `releasing/mac-vm-test/brick_test.py:125` — Retained failed tags become mandatory upgrade-test
inputs

The planner selects stable-shaped tags without proving they published installable artifacts. A retained failed attempt
can therefore become a mandatory upgrade source after a later stable release, blocking the brick-test pass. No affected
current tag was established, and intended historical-gap policy remains ambiguous. Use published-release inventory or an
explicit exclusion for proven never-published attempts, while refusing unexplained missing artifacts.

## Evidence and triage context

- releasing/mac-vm-test/brick_test.py:70–72 enumerates local Git tags; :120–138 includes every stable tag within the
  numeric range and creates an upgrade pass.
- releasing/mac-vm-test/brick_test.py:663–667 calls that planner for both plan and init, then checks candidate metadata
  rather than filtering previous tags by publication.
- releasing/AGENTS.md:287–291 says failed tag builds publish nothing and the next attempt uses the next patch version.
- releasing/mac-vm-test/brick_test.py:519–527 prevents missing checkpoints from passing; :550–552 makes any incomplete
  planned pass yield could not run.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- releasing/mac-vm-test/BRICK-TEST.md:63–67 forbids silently narrowing historical gaps, but does not distinguish missing
  artifacts of actual releases from attempts that never published. SPEC.md:2425–2428 describes upgrades from
  installations of stable releases; it does not explicitly require installing never-published tags. TODO.md:81–101 is
  Near term, not Planned, and does not settle this inventory distinction.

Caveats:

- The existence of an affected unpublished tag in the current release inventory was not established. No network
  discovery or brick test was run. The failure is a blocked validation verdict, not false acceptance of a broken
  release.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_07_cor:p1:F1`.

- `automation_website_07_cor:p1:F1`: confidence as filed: possible; suggested bucket as filed: other.

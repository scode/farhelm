# Hero publisher self-test requires different hashes for potentially identical commits

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The publisher self-test could reject two valid identical commits.

## Details

F274 — **possible** — `scripts/publish-readme-hero.sh:273` — Hero publisher self-test requires different hashes for
potentially identical commits

Two identical root commits made within the same timestamp second can legitimately have the same hash, but the self-test
requires distinct hashes. Correct second publication can therefore fail validation; same-second frequency was not
measured. Change the second image or supply distinct deterministic timestamps through child-process inputs, giving the
difference assertion an actual premise.

## Evidence and triage context

- scripts/publish-readme-hero.sh:163–176 uses the same source revision, image, identity and message for both root
  commits. The self-test publishes the same PNG at :264 and :270, then requires different hashes at :273 without
  changing any deterministic commit input.
- publish-readme-hero.sh:163–176 creates identical root-commit inputs; :264 and :270 publish the same PNG; :273 requires
  different hashes. FILTER.md:28–34 requires a rare trigger as well as safe retry. Both commits landing in the same
  second can be ordinary on a fast local repository, so FILTER.md:18–20 prevents treating the uncertain rarity premise
  as a definitive exclusion.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/FILTER.md:24–42 requires both a rare trigger and a wholly safe, retriable consequence. The
  safe-retry consequence fits, but rarity is not established; :18–20 says unclear matches do not match.

Caveats:

- Both commits must receive the same second-resolution timestamp. Frequency was not measured.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_09_sec:p1:F4`,
`automation_website_09_cor:p1:C4`.

- `automation_website_09_sec:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
- `automation_website_09_cor:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

# Duplicate scenario titles silently conflate distinct sessions

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Duplicate scenario titles make capture checks refer to the wrong session.

## Details

F234 — **definite** — `e2e/readme-hero/scenario.ts:195` — Duplicate scenario titles silently conflate distinct sessions

Scenario staging uses titles as keys despite accepting duplicates. Separately created sessions with the same title can
be selected and checked as the later one; compatible expectations allow capture to succeed with this identity
substitution. Reject duplicate titles during validation, or introduce unique scenario keys and use them consistently
throughout staging and inspection.

## Evidence and triage context

- e2e/readme-hero/scenario.ts:190-216 validates title presence, status, age, and the single open flag without enforcing
  unique titles.
- e2e/readme-hero/stage.ts:259-287 creates each session separately but overwrites ids under the title key.
- e2e/readme-hero/stage.ts:293-314 uses that overwritten mapping for status checks, seen writes, and the selected
  session.
- e2e/readme-hero/stage.ts:332 creates another title-keyed map; :144-149 applies its last matching design's directory
  and age to every row with that title.
- docs/readme-hero/SPEC.md:15-20 and :38-45 require the output to follow the scenario's design; :76-80 explicitly
  supports editing that design.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Current checked-in scenarios need not trigger the defect.
- Some duplicate combinations fail visibly; compatible duplicates can succeed with the wrong selection and rewritten
  fields.
- The consequence concerns generated capture artifacts, not ordinary product session identity.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_05_cor:p1:F3`.

- `test_infrastructure_05_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.

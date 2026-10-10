# Screenshot runs collide through shared fleet/auth files

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Overlapping screenshot commands could interfere through shared files.

## Details

F316 — **possible** — `e2e/docs-shots/paths.ts:10-16` — Screenshot runs collide through shared fleet/auth files

Screenshot runs use fixed fleet, stack, and authentication paths, and companion capture cleanup removes the same
authentication state. Two agents sharing one checkout are expressly excluded, but one operator's overlapping commands
are not clearly covered. That supported-overlap premise remains unresolved. Clarify or enforce serialization for
overlapping captures, or give each run private paths and pass them consistently through startup, capture, and cleanup.

## Evidence and triage context

- docs-shots/paths.ts:10-16 uses fixed checkout-local stack/fleet files; docs-shots.config.ts:42-43 uses the common auth
  state. readme-hero/start-stack.sh:89 and :125-126 shares and removes that auth file. AGENTS.md:663-669 states one
  agent per checkout and identifies these files as unisolated. review_feedback_queue/publisher-copy-race.md:45-46
  rejects an analogous blanket inference, but does not itself cover this mechanism.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_04_cor:p2:C7`.

- `test_infrastructure_04_cor:p2:C7`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

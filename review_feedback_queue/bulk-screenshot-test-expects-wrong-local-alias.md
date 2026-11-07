# Bulk screenshot test expects wrong local alias

Reviewed commit: 1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40

## TLDR

The bulk-action screenshot regression expects a local-host alias that its own fixture does not provide.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/docs-shots/visual-review.spec.ts:2630 at 1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40`.

Source assessment (read with the later corrections below) (review record g090/l12, event 24):

NEW verified discovery-pass-3 finding; supplement/finalization of l12 with exact caller and later-history checks.
Reviewed commit 1c5a7dfa0bf85b644da9e97cb4eb5526bb904c40. TLDR: the focused session screenshot review cannot capture its
empty-filter state because it expects Local while its own staged host is named this machine. Assigned changed assertion
at visual-review.spec.ts:2630 requires No waiting sessions on Local.; declared docs/docs-shots/scenario.json5:24
supplies local alias=this machine. stage.setup.ts:20-22 loads that scenario and calls stageFleet;
readme-hero/stage.ts:257-259 commits each declared alias through the real API. open.ts loads the same scenario.
FilterHostLabel at list/view.rs:699-701 calls Host::short_name().escaped(); lib.rs:865-881 passes stored alias to proto
host_short_name; host_name.rs:20-23 returns any alias verbatim before considering the Local fallback. Product output
therefore is No waiting sessions on this machine. as SPEC.md:222-227 requires. Docs rewrites stage-docs.ts:219-236
replace only exact remote SSH destination strings and cannot change this alias. visual-review.spec.ts:2880-2882 invokes
this path when FARHELM_VISUAL_REVIEW_SCOPE=sentence-sessions; sentence-session-hints returns before it. Correct the
assertion to the scenario's local alias or its exact expected this machine spelling, retaining aliased identity
evidence. Bucket other, confidence confirmed by source; developer screenshot tooling only, no demonstrated product
failure or highest route. proposed_drop:false; possible_cover:none searched, parent decides. No runtime capture
executed. Narrow git log over the assertion, docs fixture/setup, staging helper and short-name helper finds no later
changes between the reviewed commit and pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b, so this mismatch persists
there. This final event replaces only the previous temporal-pending qualification; substantive evidence and all previous
events remain.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g090/l12`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.

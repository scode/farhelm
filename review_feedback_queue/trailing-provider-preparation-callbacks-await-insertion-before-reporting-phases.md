# Provider preparation callbacks await insertion before reporting phases

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Provider preparation callbacks await insertion before reporting phases

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g045/c10`.

### g045/c10

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 28 (candidate, open):

# Preparation visibility does not expose a demonstrated callback insertion race

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b.

## TLDR

Checked whether publishing preparation's visible flag before inserting its step lets a provider update the following creation step by mistake. The two registered providers await needed() before sending preparation phase text, so that proposed race is not supported by the checked callers. This does not revise l3's separate overlapping-running-step defect.

## Details

SessionPreparation::needed sets visible with swap and then awaits the memory lock before insertion (tensorlake_add.rs:57-74); step chooses index from visible (:77-81). Tensorlake's ensure_prepared awaits progress.needed() before entering the build helper that receives its progress.step closure (tensorlake/hosts.rs:218-226). Podman's ensure_prepared likewise awaits needed before starting/joining its job, and reports phase text from its sequential job loop afterward (environment/podman.rs:994-1029). Each launch owns its own SessionPreparation value (:516-520); no separate task is given that value to call step concurrently while needed is inserting. Repeated provider loops await needed serially. Suggested bucket: other for the considered progress-accounting issue. Could-be-highest: no security or lost-work consequence considered. Confidence: confirmed source refutation for both shipped providers, no execution. proposed_drop: callers await insertion before the phase callbacks at issue. possible_cover: none; local l3 concerns presentation despite correctly inserted steps, not this callback race. Caveats: the adapter does not independently guarantee correctness for a future provider that invokes these callbacks concurrently. No tests, builds, reproductions or live actions performed.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g045 record](records/review-through-00de0b6c/groups/g045.json). Record ID: `c10`.


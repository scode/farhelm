# Restart template plan overstates the old refusal

Reviewed commit: 2cffab3f03ac1a076f19d189d74597dd8c9c2450

## TLDR

The plan says no saved template can apply in Restart with, although kindless older templates can already apply editable
fields. This is a background explanation error; its implementation criteria remain useful.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `plans/queue/restart-with-template-refusal.md:22-24`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 2cffab3f03ac1a076f19d189d74597dd8c9c2450. TLDR: the plan tells its executor that all saved templates
are currently unusable in Restart with, although supported older templates can still supply editable launch settings.
Details: original plan lines 22-24 say every template made in the Templates editor stores launch kind and consequently
no saved template can be applied at all. SPEC.md:747-749 explicitly preserves stored kindless templates and offers a
don't-switch editor mode. crates/farhelm-ui/src/list/templates.rs:797-807 infers a missing kind only in an editor draft,
without updating the stored record. The existing allowed partial-template test at
e2e/tests/restart-with.spec.ts:1826-1847 applies kindless model/effort/permission/wrapper fields. The affected baseline
is saved launcher snapshots and templates that actually contain held fields, not every saved template. Correct the
background statement to preserve that distinction; no product implementation change is implied. Suggested bucket: other
(plan/documentation accuracy, no security or user-work loss route). Confidence: confirmed by source inspection. Caveat:
this does not invalidate the plan's acceptance criteria or main implementation approach; no runtime behavior was
reproduced. possible_cover: none identified. Focused pinned-tip searches of SPEC.md, SPEC_impl.md, TODO.md's empty
Planned bucket, BUGS.md, review_feedback_queue/INDEX.md and TRIAGE_OUTCOMES.md found no coverage of this erroneous plan
assertion. The specs' kindless-template compatibility rules establish the assertion's inaccuracy; they do not accept the
inaccurate documentation. The ledger's failed-template-read finding has a different trigger and consequence. No filter
fits a static false baseline statement in this plan. proposed_drop: no.

The later equal-held-field implementation has landed at recording time. This finding concerns the retained historical
plan's background assertion about kindless legacy partial templates, not the fixed refusal of templates repeating held
values. No current runtime refusal regression is asserted.

Provenance: `g125/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.

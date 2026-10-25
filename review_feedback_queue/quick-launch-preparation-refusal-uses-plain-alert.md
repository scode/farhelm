# Quick-launch preparation refusals use the old plain alert

Reviewed commit: 1de332971ec47040f095b0fbc32903fbcb88ae73

## TLDR

A quick-launch preparation read failure bypasses the new bordered refusal presentation. Whether the presentation
requirement or diagnostic filter covers this stage is unresolved; request admission stays safe.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/create_form.rs:5460`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 1de332971ec47040f095b0fbc32903fbcb88ae73. TLDR: quick launch that cannot read the host's recent setups
still shows a plain error line, without the new bordered Not launched warning. Cross-commit route: the assigned
4ae55cc50dafcae6836abe835939ca79034cbdf7 patch adds e2e/tests/quick-launch.spec.ts:106-130 to require showing this
failure rather than launching into a guessed folder. At 1de, create_form.rs:4240-4248 writes preparation_error and
reveals New;5460-5462 renders that slot as a plain p outside the new refusal branch at5561-5584. The message retains
role=alert, so announcement is preserved; the missing pieces are the border, warning icon and Not launched prefix beside
Launch required by SPEC.md:914-919 for every quick-launch refusal. Neighbor launch-refusal.spec.ts:94-124 explicitly
treats asynchronous preparation failure before dispatch as a refused Launch press; therefore the initial-pass
categorical dismissal of preparation failures was too broad. Suggested bucket: other. Confidence: confirmed source path
and presentation mismatch; contract terminology could distinguish setup failures from refused launches, but no explicit
acceptance of this exception was found. No runtime frequency measured. No security, wrong-target mutation or loss of
user work established. possible_cover: SPEC.md:916-917 exempts errors that are not refused launches; whether that
includes this quick-launch abort is the remaining acceptance ambiguity. Pinned searches of SPEC.md, SPEC_impl.md,
TODO.md Planned (empty), BUGS.md, TRIAGE_OUTCOMES.md, queue INDEX/feedback and plans/queue found no matching accepted
exception or queued fix. proposed_drop: none recommended on present evidence. FILTER's rare-diagnostic rule is a
possible basis only if its rare-trigger premise is established; this path follows any recent-setups read error, without
requiring a narrow timing window.

The upcoming Steel dialog plan changes layout and shared anatomy while explicitly keeping behavior and keyboard
contracts unchanged. It does not explicitly route preparation failures through the bordered refusal renderer, so
coverage remains uncertain rather than assumed from a general redesign.

Provenance: `g243/f3`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.

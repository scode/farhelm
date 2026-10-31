# Container off/on discards unsaved editor values

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Turning a template's container option off and back on loses its image, port and token-path draft values before Save.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The editor clears dormant buffers when the toggle changes. This is routine draft loss the user can recreate, not loss of
persisted work. The wire-reset contract may explain saved-state clearing but does not clearly settle retaining an
unsaved off/on editor draft. Invalid lenient projection and history write-back were separately source-refuted.

Source locations: `crates/farhelm-ui/src/list/templates.rs:1230-1239`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis revision, reviewed d6645f771bee8ed03a66486ef8755834937d759d; same original lines in
9a15ddfcdb37605961da9237de16c22656af8771. Exact ordinary user sequence: open a template with valid container settings;
uncheck Containerized; without saving, check it again. Off converts the raw choice to fields.container=Some(None) and
drops all raw input. On reconstructs Default from None, then stores Enabled(true) as container_input because its empty
port is invalid. The checkbox renders checked and all three inputs render empty; Save refuses until the required fields
are reentered, disabled again or the field removed. The same loss occurs if off is used while correcting an invalid raw
container draft. No asynchronous interleaving is needed. Correction to any possible stronger reading of turn 1: the
saved template is not changed by the toggles alone; Discard/reopen can restore its saved settings. Saving while off
deliberately persists null, which correctly means turn isolation off when applying the template. The finding concerns
the separate temporary off/on editor behavior. Component and direct-draft contract: container_controls.rs:11-13,215-230
preserve dormant values; launcher owner does preserve them. Template owner retains raw draft only on Err, so it violates
that reused draft behavior. Bucket other; confidence confirmed source route. proposed_drop none. possible_cover
narrowed: SPEC.md:733-735 and SPEC_impl.md:6255-6264 explicitly define null as an off reset and use of direct wire
fields, but do not explicitly require temporary off to erase editor buffers, forbid retaining dormant values outside the
wire field, or accept this loss. Existing raw wrapper/container buffers already sit outside fields. No explicit accepted
UI loss contract found. Mounted test saves off immediately (templates.spec.ts:1083-1090), so it proves null semantics
but not off/on retention. No runtime execution performed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g121/f1`, `g123/f5`, `a022/g121-f1`. These are
original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts
remain in the private review evidence. No fix or triage decision was executed.

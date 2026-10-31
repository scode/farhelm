# Checkout refusal conflicts with the failed-save rule

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The general failed-Save discard rule does not explain the explicit checkout-refusal exception that retains the draft.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

SPEC.md:1134–1138 and SPEC_impl.md:854–869 explicitly retain a refused checkout draft, while SPEC.md:3845–3848 broadly
says failed GUI saves discard attempted drafts. Actual UI behavior retains the checkout draft on validation/transport
refusal. This is a contract wording conflict, not an established code defect requiring removal of the specific
exception.

Source locations: `SPEC.md:1134-1138,3845-3848; SPEC_impl.md:854-869`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis complete for d6645f771bee8ed03a66486ef8755834937d759d; assigned commit
08aaac8a4a5bcf91726a1d3bfcfc980de4116971. TLDR: the specifications give different answers about keeping entered checkout
settings after an unsuccessful Save. Corrected source basis: SPEC.md itself explicitly says the launcher uses the same
checkout-folder field and refusal keeps its draft editable (:1134-1138); SPEC_impl.md:854-869 specifies retained refused
drafts for folder and post-clone command. SPEC.md:3845-3848 nevertheless says a GUI save failing for any reason discards
its change and the next view or save starts from the helm. The tests retain an invalid folder and oversized command
across section navigation (settings.spec.ts:1255-1265,1545-1554 at tip; :1152-1162,1442-1451 in the assigned commit).
Actual checks: server handlers checkout_config.rs:976-993,1026-1043 validate before any setter and return 400 on
validation refusal; UI api.rs:604-618,646-658 turns non-success responses into Err(String) and propagates transport
errors into that same result; api.rs:1993-2007,2120-2124 distinguishes request/authentication internally but flattens
display errors; checkout_root.rs:95-105 and checkout_post_clone.rs:119-128 retain draft/edited state for all Err results
and refetch only on success. Thus shipped code does not implement a validation-versus-transport draft distinction, but
that is not automatically a code bug because the narrow retention contract is explicit. No automatic retry,
success-on-error, credential or user-work-loss route established. Searches across specifications, BUGS, TODO, queued
plans, queue and ledger found no explicit checkout refusal exception to the general failed-save rule. Relevant ledger
decisions (:17602-17624,17668-17694,17977-17988,18520-18533) concern preference/binding persistence and sign-in replay,
not this explicit-save field. The separate template Delete decision (:18918-18944) distinguishes transport failures from
helm refusals only for template deletion. TODO.md:140-167 says refusals are replies rather than failures within future
infallible-connection work, explicitly asks to settle the relation to the failed-save rule, and is Near term, not
Planned; it is not coverage or a waiver. Other explicit refusal-retention wording at SPEC.md:1652-1658 corroborates that
the broad rule may need narrower wording; no additional finding added. This remains a pre-existing textual/interface
ambiguity, not a regression caused by casing. Suggested bucket: other. Confidence: confirmed wording tension and shared
error handling; intended exception/precedence unresolved. proposed_drop: none. possible_cover: SPEC.md:1134-1138 and
SPEC_impl.md:854-869 explicitly accept retaining validation refusals; do not present retention itself as independently
defective. Remaining missing premise is policy intent, not runtime reachability; source cannot resolve two authoritative
statements, and an explicit exception/definition or maintainer decision would settle it. No tests, experiments or
historical private evidence authentication. f1 remains withdrawn only as exact existing coverage, with every original
event preserved.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g037/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

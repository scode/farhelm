# OMP documentation retains the retired YOLO default

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An OMP comment and implementation specification still describe the old default permission mode after new launches
changed to Guarded.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The compaction/compiler path now selects the new default, while the nearby rationale and SPEC_impl.md:4033–4036 retain
the earlier contract. This concerns new-launch documentation; historical stored omissions are a different case. The
recorded intended fix does not explicitly accept the remaining contradiction.

Source locations: `crates/farhelm-ui/src/list/create_form.rs:6701-6704; SPEC_impl.md:4033-4036`. Historical revision
qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d. TLDR: OMP currently displays and launches with the correct
Guarded default, but its recent-row explanation and an implementation-spec paragraph still say omission means YOLO and
Guarded must stay explicit. Maintainers are given contradictory contracts for the same permission choice. This is a
concrete false rationale about an existing invariant, not a request to add more comments. No current product-permission
defect is claimed. Evidence: create_form.rs:6701-6704 explicitly exempts OMP from Defaults compaction because its
default is supposedly YOLO. Classification at 6612-6618 instead compares omitted_permission and request_permission to
Guarded. farhelm-proto/src/launch.rs:267-320 includes Omp in Guarded defaults; stored_permission at 323-348 deliberately
retains YOLO only for historical stored omissions, with the distinction pinned by the test at 932-964. The changed
browser test sidebar.spec.ts:7225-7244 requires a Guarded OMP row to say Defaults and its tooltip to say Guarded.
launch_composer.rs:3204-3239 also asserts the omitted OMP summary is Guarded. The helm normalizes a new omission before
compilation at launches.rs:302-338; its test at 1296-1320 expects literal omp --approval-mode write, using a helper with
permissions None at 1214-1224. SPEC.md:668-672 and SPEC_impl.md:3904-3917 require Guarded for new OMP requests. In
contrast, SPEC_impl.md:4033-4036 still says an omitted permission normalizes to YOLO and explicit Guarded remains
explicit. That paragraph describes structured launch compilation, so the separate historical stored-omission contract
does not explain it. Suggested correction: state the current request-default rule and remove the false OMP compaction
exception; preserve the documented historical stored-omission distinction. Provenance: inherited-context finding
investigated during g039 synthesis at the parent request. At exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c the
source comment already exists at create_form.rs:6575-6578, beside Guarded classification at 6486-6492, and the
contradictory spec paragraph exists at SPEC_impl.md:3772-3775. It is not introduced by assigned casing commit
08aaac8a4a5bcf91726a1d3bfcfc980de4116971. Suggested bucket: other. Confidence: confirmed by source; no missing factual
premise. proposed_drop: none. possible_cover: TRIAGE_OUTCOMES.md, guarded-choice-becomes-omp-yolo.md, 13500-13535,
records the intended default change and claims both specs updated. This settles the intended behavior but does not
explicitly accept the surviving contradictory documentation; original consequence was an automatic permission
escalation, whereas this finding is stale guidance after that product repair. Coverage searches completed across both
specs, empty TODO Planned bucket, BUGS.md, plans/queue, queue index and relevant queue searches, and TRIAGE_OUTCOMES.md;
no exact duplicate or accepted exception found. The default-approval-reset-renews-retry-key.md ledger entry at
14324-14359 preserves historical OMP/Goose wording while addressing retry identity, not this comment/spec correction.
SPEC.md First-class harnesses at 3973-3985 accepts partial activity/session integration, not contradictory
default-permission guidance. FILTER.md has no applicable filter for this ordinary, static documentation defect. No
security impact, data loss, product malfunction, or test false pass is established; future regression from following the
stale text is a risk only. No tests ran, and no historical private runtime evidence was authenticated.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g039/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

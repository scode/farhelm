# Template-save placeholder validation scope is unclear

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Template Save can retain a prefix that Launch later refuses, despite wording that the new placement check runs at Save
time.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The shape-only template contract and new save-time placement promise have conflicting scope. The review investigated
actual Save/Launch validators; it did not establish which contract should prevail. Launch still refuses before stopping
the source in the inspected route. No unauthorized execution or work loss is claimed.

Source locations: `SPEC.md:754-758; SPEC_impl.md:1910-1912,6183-6186; crates/farhelm-helm/src/templates.rs:98-109`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Record-repair pass only; no new source search or runtime claim. Reviewed commit remains
d6645f771bee8ed03a66486ef8755834937d759d. Parent accepts the existing other-bucket normative save-scope conflict. Source
behavior remains confirmed from prior passes, while the intended scope of 'refused on save' remains unresolved: template
writes use shape checks and complete native command-launch admission applies placement checks. The explicit shape-only
template-write rule at SPEC_impl.md:6183-6186 remains possible_cover; coverage is ambiguous rather than settled. Partial
templates remain distinct from invalid placeholder spelling. Confidence: confirmed behavior, specification intention
unverified after investigation. The checks/results, exact missing normative premise, and resolving evidence are
unchanged from the prior f1 synthesis revisions: accessible paths establish behavior, and explicit product-spec
interpretation would settle whether template-definition saves are included. Suggested bucket other; proposed_drop=false.
No new finding ID or additional consequence is introduced.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g183/f1`, `a012/f7`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.

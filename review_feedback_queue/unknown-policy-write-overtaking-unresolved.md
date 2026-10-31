# Unknown policy-write overtaking remains unresolved

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An uncertain opt-in might outlive a later opt-out, but a concrete older request delayed before policy-lock admission
remains missing.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Matching reqwest/hyper/axum/Tokio and policy-writer source were inspected. A delay after taking the FIFO policy lock
cannot supply the claimed inversion. No shipped pre-lock producer was established for the remaining sequence. Preserve
that exact gap rather than treating generic delayed database writes as proof. Committed opt-out and cancellation
ownership exclusions remain independently supported.

Source locations:
`crates/farhelm-ui/src/api.rs:1513-1548; crates/farhelm-helm/src/middleware.rs:51-74; crates/farhelm-helm/src/preferences.rs:217-267`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Parent reports a011's independent source review keeps coverage AMBIGUOUS after matching transport/mutex checks. Preserve
f3 as retained, unverified after investigation; proposed_drop is now none, rather than a proposed exclusion based on
general unknown-outcome acceptance. Exact remaining premise: a supported producer/schedule allowing an older true policy
request to reach automatic_update_admission only after a newer false policy request has acquired it and committed. No
such shipped pre-lock overtaking producer or resulting unauthorized update has been established. All prior checked
source sites and limitations remain: owned extraction can await body EOF, combined tunnel patches can await the tunnel
gate, but Settings emits finite one-field JSON and no application retry; a delayed DB worker after acquiring policy
cannot be overtaken by restoration/opt-out. possible_cover remains SPEC_impl.md:252-253, whose scope against
SPEC.md:152-153 and SPEC_impl.md:779-783 is ambiguous. Suggested bucket other for established unknown-outcome behavior;
no highest consequence route is established, and the possible consent interpretation is retained for parent judgment.
Confidence remains investigated uncertainty, missing the exact producer/scheduling premise. Provenance: original
g141/f3, this reviewer's synthesis checks, and parent-reported a011 independent AMBIGUOUS disposition; no private
runtime evidence was authenticated.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g141/f3`, `a011/f3`, `a039/x-f3`. These are
original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts
remain in the private review evidence. No fix or triage decision was executed.

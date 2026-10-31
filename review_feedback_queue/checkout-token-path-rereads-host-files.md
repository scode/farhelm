# A checkout-controlled token path can disclose host files

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

If the configured token path lies in the writable checkout, a later container tab can receive a readable host file
selected through that path.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

container.rs:112–128,316–324 reads the configured path on the host and forwards the resulting text as GH_TOKEN. The
whole checkout is writable in the container, but this later host read has no matching no-follow/type/confinement check.
The configuration and later tab receiver are supported source paths; the route requires the selected token path or an
ancestor to be controlled through that checkout and the outside file to be readable text. No arbitrary host-file access
without those conditions is claimed. Independent reviewers retain this route. a002/f12 initially stopped, then a later
source-only pass checked the normal validation/admission/receiver boundaries and reconciled the exact duplicate; it does
not claim new independent verification of the full consequence.

Source locations:
`crates/farhelm-supervisor/src/container.rs:112-125,316-324; crates/farhelm-supervisor/src/launch.rs:1286-1300`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g119/f2`, `g019/f3`, `a022/g119-f2`,
`a024/g019-f3`, `a002/f12`. These are original reviewer identities for this concern, not additional bugs. Full
revisions, withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was
executed.

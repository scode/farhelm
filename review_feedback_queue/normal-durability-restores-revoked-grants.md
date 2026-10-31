# Power loss can restore acknowledged permission revocations

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A power loss can restore a host command grant or browser network/tunnel permission after Farhelm acknowledged its
revocation.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The WAL/NORMAL policy introduced by 678d42a7e97ce44ccd5aad2a88bdfe3900c4eff1 allows loss of the acknowledged transaction
that stores these revocations. Existing remote requester/target processes or browser credentials can survive the helm
restart, making the restored grant relevant to later admission. The tunnel route still requires a fresh deliberate
tunnel gesture; it does not resurrect old listeners. Token rotation and initial identity publication select stronger SQL
barriers and are separate from this defect. Broad recent-state-loss acceptance in SPEC_impl.md:3996–4008 conflicts with
the specific revocation and consent guarantees; it is not silently treated as coverage. These are established
conditional source routes, with no physical power-cut observation.

Source locations: `crates/farhelm-helm/src/store.rs:4520;6444-6473`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g177/f3`, `g165/f3`, `g166/f1`, `a017/g177-f3`,
`a017/f1`, `a017/f2`. These are original reviewer identities for this concern, not additional bugs. Full revisions,
withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was executed.

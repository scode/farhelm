# Full host-name tooltip promise conflicts with its line cap

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The promise to reveal full host names conflicts with the shared tooltip's eight-line limit; actual clipping remains
unmeasured.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

SPEC.md:1599–1600 promises full host names, while SPEC_impl.md:1203–1207 and app.css:1778–1800 impose an
eight-line/280px bound. Accepted formatting-character aliases can expand substantially when displayed. Generic tooltip
limits do not settle the newer full-value promise. Accessible labels retain full text; actual font wrapping/clipping was
not observed.

Source locations:
`SPEC.md:1599-1600; SPEC_impl.md:1203-1207; crates/farhelm-ui/assets/app.css:1778-1800; crates/farhelm-ui/src/hosts.rs:666-670,709-727`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d; assigned change 0ea8f3872ccc53b3c1f02e9c4d9e2fb2e9e531ca.
Discovery finding, independent of f1 width incompatibility. TLDR: a sufficiently long displayed host identity can be
truncated in both the expanded row and its tooltip despite the rail's new full-name hover promise. Confirmed
source/specification conflict; no reproduced native failure. SPEC.md:1599-1600 promises full long names in the tooltip.
hosts.rs:666-670,709-727 composes escaped name, distinct destination and phase/attention text; app.css:1778-1800 limits
the shared tooltip to 280px, eight lines and hidden overflow, matching SPEC_impl.md:1203-1207. Producer checks now
include HostDestinationForm (hosts.rs:4988-5026), which takes ordinary text input into the raw draft and submits that
value; alias validation (3302-3313) and durable validation (farhelm-helm/src/store.rs:1597-1615) accept at most 64
non-control characters, without a display-length limit. farhelm-proto/src/text.rs:18-25 explicitly distinguishes Cc
controls from the bidi formatting characters; 57-79 classifies U+202E as presentation-unsafe separately from is_control.
peer.rs:159-180 expands each such character to <U+202E>. A GUI alias consisting of 64 U+202E characters therefore has a
supported source path to 512 visible ASCII characters before tooltip state/destination suffixes. Whether this concrete
spelling paints beyond eight lines with the shipped font and engine remains unobserved. Actual accessible name retains
full text, and Hosts/settings are alternate access; no wrong-target, security or user-work-loss consequence established.
Suggested bucket other. Confidence confirmed source contract conflict; actual clipping unverified after investigation.
Checks performed: producer input/draft, both validators, exact presentation character classification/escaping, tooltip
textContent insertion (tooltip.js:297-309) and CSS cap. Missing premise: concrete font/engine wrapping and visible clamp
result. Source alone cannot observe native painting. Resolving evidence: retained tooltip geometry/screenshot for the
accepted alias, or an explicit specification decision bounding the rail promise by the shared cap. Coverage searches of
both specs, empty TODO Planned, BUGS, plans/queue, queue index and ledger found no explicit reconciliation.
proposed_drop none. possible_cover SPEC_impl.md:1203-1207 explicitly accepts eight-line tooltip clamping; ambiguity
against SPEC.md:1599-1600 is retained rather than silently resolved. The earlier 4096-byte destination observation is
not needed for this finding and supplies no claim about a real SSH configuration.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g175/f2`, `g174/f3`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.

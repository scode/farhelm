# Typed-name matching repeats quadratic prefix work

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Typed template matching performs quadratic duplicate checks before applying the five-row display cap; practical
supported-scale latency remains unmeasured.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The Vec contains checks can perform k(k-1)/2 comparisons before presentation truncation. Source inspection confirms the
cost shape, but no supported catalog/input size and actual user-visible latency were measured. This is not a
demonstrated denial of service or a reason to truncate ambiguity resolution to the display cap.

Source locations: `crates/farhelm-ui/src/launch_sequence.rs:527-550,731-739,927-964`. Historical revision qualifiers in
those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Additional discovery revision of a031/d2 at reviewed d6645f771bee8ed03a66486ef8755834937d759d. The confirmed source-cost
finding and investigated latency limits in the preceding revision stand: a broad prefix with k distinct matching saved
names performs k(k-1)/2 membership comparisons before the five-row display cap; indicator render, Tab and Enter all use
this helper; store listing supplies the complete unique-name catalog. Practical latency at an in-scope catalog size
remains unverified after investigation, with no benchmark or security/work-loss claim. Further test inspection at
launch_sequence.rs927-964 explicitly requires duplicate input names not to manufacture ambiguity. Therefore do not fix
d2 by simply dropping duplicate handling from the helper contract: preserve stable tier/catalog order, exact/folded
precedence, every winning-tier ambiguity competitor and duplicate-name semantics, for example using a constant-time seen
set. Ordinary database names remain unique (store.rs396-400), but helper/test compatibility is a separate constraint.
Bucket OTHER for demonstrated quadratic work; confidence confirmed cost, practical impact unmeasured. proposed_drop
false; possible_cover only adjacent closed-popovers repeated-work concern and oversize-catalog/terminal-library
decisions, none an exact match. Parent's g157/f2 synchronous-eval-clear exclusion goes to fresh a049 and is not
self-certified by this discovery. No further defect was established by the subsequent refresh/refusal fixture and
rendering inspection. No checkout edits or runtime experiments.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a031/d2`, `a049/f10`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.

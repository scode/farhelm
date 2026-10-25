# Ordinary host setup discards a waiting agent dialog

Reviewed commit: 579783dd50938967b2c851b8d0b2f855df48c978

## TLDR

Submitting ordinary host setup can clear a deferred agent Add-host opening that was already seen. Its approval card
remains available for manual reopening; automatic presentation is lost.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/hosts.rs:2084-2095`. Locations and revisions in the assessment below identify
the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 579783dd50938967b2c851b8d0b2f855df48c978. Preserved at pinned tip
463cac1528b5c5e259404975380d321a3fff0412 with the same line numbers. TLDR: an agent's scheduled Add dialog can disappear
when the user confirms setup for a different host they were already adding; the pending request survives on its card and
can be reopened manually. Concrete sequence: user opens ordinary Add (hosts.rs:2223-2246), reaches its setup question,
and stays in that focused window. A new HostAdd request is successfully listed, marked seen and queued
(approvals.rs:150-176). Its independent effect removes the first queued candidate and stores it in opening even while
the ordinary form is present (213-217). hosts.rs:2266 prevents presentation while adding is true. These states can
remain for the whole ordinary setup question; no sub-second or stale-listener sequence is needed. Confirming ordinary
setup invokes the shared callback with from_agent_request=false and unconditionally clears opening (2084-2095), then the
POST finishes and add_submission_pending clears (2118). The request remains seen, so subsequent listings do not
reconstruct its automatic candidate. Its card's explicit action can queue it again (approvals.rs:302-308). SPEC.md at
the reviewed commit 2783-2788 and SPEC_impl.md 5678-5683 require keeping the draft and presenting focused queued
requests after it closes; pinned-tip SPEC.md:2783-2788 and SPEC_impl.md:5685-5690 preserve this contract. Suggested
bucket: other. Confidence: confirmed source mechanism; runtime frequency unmeasured. No lost agent request,
installation, wrong-target mutation, security issue, or user-work loss is established. possible_cover: no exact coverage
established; initial pinned-source search found empty TODO Planned, no matching BUGS/INDEX/ledger acceptance, and the
existing two host-dialog queue items concern different modal lockouts. The person-in-subsecond filter does not match
this sustained state. Rare/self-correcting filter remains only a possible argument if rarity were established and manual
card reopening treated as retry; neither is established as complete filter coverage, so no exclusion is proposed.
proposed_drop: false.

Provenance: `g300/f3`, `g297/f1`, `g298/f4`, `g299/f2`, `g301/f1`. These are reviewer-local identities, not additional
findings. Proposed drop: none for the retained remainder.

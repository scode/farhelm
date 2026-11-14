# Environment wait comment promises a relay ordering the budgets do not guarantee

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Environment wait comment promises a relay ordering the budgets do not guarantee

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g020/l7`.

### g020/l7

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 25 (finding, open):

# Environment wait comment promises an unguaranteed relay ordering

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435

## TLDR

The new environment-launch explanation tells maintainers that the helm's still-launching answer reaches the agent before
its relay deadline. The two timers start at different phases, so that is only the normal outcome, not a guarantee. Slow
work before approval or registration can consume the spare relay budget first. The existing unknown-outcome response is
still correct; this finding is the misleading timing explanation.

## Details

The assigned new comment at agent_requests.rs:4550-4554 says the relay budget covers approval plus the local launch
allowance, ‘so answering by the end of the allowance after the Allow reaches the agent before its relay gives up’, with
the still-launching explanation ‘instead of a lost answer.’ The local timeout at :4555-4557 starts after both approved
(:4493-4515) and an awaited start_session_environment registration (:4541-4549). Registration itself performs
store/provider reads and an unbounded add_confirmations.lock_owned await (provisioning/tensorlake_add.rs:357-390) before
returning its result receiver. Pre-card planning/source discovery also precedes approved.

The relay instead composes 30 seconds ordinary answer budget plus nine-minute approval allowance plus thirty-minute
environment allowance (service/agent_relay.rs:640-663; service/core.rs:312,476-478; proto/approvals.rs:24-45).
HelmLink::upcall starts this one answer timer once the request is QUEUED on the connection, at agent_relay.rs:317-326,
before the helm's source/planning/approval/registration phases. The approval deadline starts later when Approvals
inserts the card at approvals.rs:485-538. There is no shared monotonic deadline passed to the local environment timeout
and no budget subtraction for those intervening phases. If pre-card plus post-Allow pre-timeout work exceeds the
ordinary 30-second spare plus unused approval allowance, the relay deadline precedes the local thirty-minute timeout. A
near-end-of-window Allow and slow source/registration phases suffice for the timing inequality; no runtime incidence or
exact delay is asserted.

This limitation is explicitly part of the governing contract, rather than a request to tighten timers:
SPEC_impl.md:9390-9396 says the relay's extra thirty seconds is ‘an allowance for the work the helm does before and
after the wait, not a guaranteed ordering’, and gives slow clone source resolution as an example. The
environment-specific paragraph at :6692-6698 carefully says the agent ‘normally hears that before its relay gives up’.
SPEC.md:4336-4340 and proto/approvals.rs:35-44 count approval and launch against one span and retain unknown outcome
while owned work continues. The new comment omits this phase-origin caveat and makes a guarantee those contracts
expressly do not make. Qualify it to normal delivery with remaining relay budget and name the different timer origins;
do not claim that the local timeout itself guarantees its message wins.

Attribution: the full 190d844e assigned patch adds this comment and its local timeout together; the parent has no
launch_environment_for_agent/ENVIRONMENT_LAUNCH_WAIT path. The complete relevant-path diff from that commit to inclusive
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db has no later change to these timeout/comment lines. No pre-range implementation
bug is attributed to this range.

Suggested bucket: other (internal timing-contract documentation). Confidence: confirmed from timer starts, additive
budgets, call ordering and explicit governing contract; no execution, reproduction or build. Could-be-highest: no
established security or user-work-loss route; lost detailed explanation still produces the specified unknown outcome and
does not cancel the task. proposed_drop: false. possible_cover: none in the complete refreshed interface records;
supplied g123/l1 concerns a separate Delete admission/decision deadline and g026/g027 browser-gate findings concern
different test waits. Caveats: normal fast preflight/registration and early Allow leave ample time, so this does not
allege common premature timeout or demand a runtime behavior change. No missing source premise remains for the
documentation mismatch.

Named interface synthesis before discovery continuation: read complete g005/f2 event7 (reviewed
b106c0e49646ecb9a17e1a06b17a70da0ece73e0). That record reports SPEC.md receipt-relative A+L/approval-consumes-launch
wording versus SPEC_impl fresh-L-after-Allow and O+A+L relay model. It explicitly corrects prior minimum-wait examples
and denies a universal answer-order guarantee. l7 reports the NEW implementing caller comment at 190d844e:4550-4554 that
independently claims the local detailed answer wins over that relay. The trigger/model foundation is shared (distinct
timer origins and pre/post-approval work), while the false statements differ: g005/f2 concerns effective launch
allowance versus approval duration; l7 concerns guaranteed order of the two answers. Correct production timeout
semantics refute neither documentary error. A single underlying deadline-explanation review item can carry BOTH
manifestations if it retains g005’s SPEC.md:4336-4338/SPEC_impl.md:6692-6697 sources and l7’s NEW
agent_requests.rs:4550-4554 location/190d844e provenance, and corrects both the allowance model and the unqualified
answer-order claim. Merely qualifying the caller comment does not fix the specification-model disagreement, and merely
aligning the spec’s effective deadline model does not remove the caller’s unconditional ordering assertion. This is a
proposed common remediation/consolidation scope, NOT a parent duplicate/exclusion decision. Confidence remains
source-confirmed documentation defect, other bucket; original no-runtime-bug/no-minimum-wait limits preserved.
proposed_drop:false. possible_cover now includes related g005/f2 event7 for one deadline-explanation item, with the
distinct new wording retained. Parent says g005/g018 reciprocal synthesis must precede the next new search; checking the
refreshed interface state next without redoing deadline source reads.

Reciprocal synthesis gate resolved: read g005/f2 event14 in full and g018/f5 event15 in full. Both independently
preserve the receipt-relative versus fresh-L model disagreement and corroborate l7 as the distinct unconditional
caller-order claim. g005 retains both as related documentation assertions for parent consolidation; g018 records exact
l7 ownership without adding a duplicate. All named syntheses are now present. This changes no consequence, confidence,
bucket, source premise or proposed_drop; the proposed single deadline-explanation item still requires both corrections
and both revision/location provenances. No runtime behavior or universal ordering/minimum wait is alleged. Discovery
continuation may now search beyond both known deadline records.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g020 record](records/review-through-00de0b6c/groups/g020.json). Record ID: `l7`.

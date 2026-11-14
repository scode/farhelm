# environment launch contracts describe different effective waits

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The product specification describes one receipt-relative approval-plus-launch span, while the
implementation specification describes an additional fresh launch allowance after approval. That gives future
maintainers different effective-deadline models. The supplied later source comment makes a separate, stronger claim that
the local still-launching reply must beat relay expiry; actual phase ordering does not guarantee that. Both are
documentation issues, not evidence that an unknown outcome is a runtime failure. Original specification discrepancy: At
b106c0e4, complete SPEC.md:4289-4341 and SPEC_impl.md:6658-6702 were read. SPEC.md:4336-4338 says the wait covers nine
minutes of approval plus thirty minutes, 'counted together from when the request reaches the helm, so a slow answer
leaves less time for the launch'. SPEC_impl.md:6692-6697 gives the relay ordinary+approval+launch in one span, then
bounds the helm's task wait 'to the same allowance after the Allow'. The full surrounding subsection does not describe
the earlier local cutoff in SPEC.md or turn the fresh launch timeout into the remaining receipt-relative budget.
SPEC_impl.md:3-6 gives SPEC.md precedence on observable behavior. Suggested remedy remains to choose one intended
effective deadline model and make both documents accurately describe it. Preserved correction and uncertainty:
Thirty-nine minutes is not a minimum guaranteed wait and earlier refusal does not violate such a guarantee. The
documentation mismatch is the asserted time model and the sentence that approval consumes remaining launch time. Let
O=30 s

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g005/f2`.

### g005/f2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: b106c0e49646ecb9a17e1a06b17a70da0ece73e0. Late interface synthesis: exact supplied g020/l7 event 23,
reviewed implementation comment commit 190d844e58c96cf197aa838d0b0eea8062f48435. Original f2 identity, consequence and
prior no-minimum-wait correction are retained; the new comment is g020/l7's independently owned location, not a new g005
ID.

TLDR: The product specification describes one receipt-relative approval-plus-launch span, while the implementation
specification describes an additional fresh launch allowance after approval. That gives future maintainers different
effective-deadline models. The supplied later source comment makes a separate, stronger claim that the local
still-launching reply must beat relay expiry; actual phase ordering does not guarantee that. Both are documentation
issues, not evidence that an unknown outcome is a runtime failure.

Original specification discrepancy: At b106c0e4, complete SPEC.md:4289-4341 and SPEC_impl.md:6658-6702 were read.
SPEC.md:4336-4338 says the wait covers nine minutes of approval plus thirty minutes, 'counted together from when the
request reaches the helm, so a slow answer leaves less time for the launch'. SPEC_impl.md:6692-6697 gives the relay
ordinary+approval+launch in one span, then bounds the helm's task wait 'to the same allowance after the Allow'. The full
surrounding subsection does not describe the earlier local cutoff in SPEC.md or turn the fresh launch timeout into the
remaining receipt-relative budget. SPEC_impl.md:3-6 gives SPEC.md precedence on observable behavior. Suggested remedy
remains to choose one intended effective deadline model and make both documents accurately describe it.

Preserved correction and uncertainty: Thirty-nine minutes is not a minimum guaranteed wait and earlier refusal does not
violate such a guarantee. The documentation mismatch is the asserted time model and the sentence that approval consumes
remaining launch time. Let O=30 seconds ordinary relay allowance, A=9 minutes approval allowance, L=30 minutes local
launch allowance. The relay clock starts at queue acceptance, so its deadline is tQ+O+A+L. The local clock starts only
after planning/source resolution, approval and awaited environment registration, so its deadline is tH+d+a+r+L. With
ordinary prompt resolution/registration, the local fresh L wins and actual approval consumes none of that local L;
sufficiently slow pre-timeout work can instead make the relay win. Neither clock guarantees a minimum wait, and neither
expiry cancels the owned launch. No complete deployed-delay incidence or runtime occurrence was measured or alleged. At
b106c0e4 this was a documentation-only design change, with no ENVIRONMENT_LAUNCH_WAIT implementation yet; later code is
supplemental caller evidence rather than then-shipped behavior.

Late named input source checks: Read only g020/findings.jsonl event 23 plus complete own history, not other g020 records
or prohibited coverage. At 190d844e agent_requests.rs:4493-4515 awaits approved, :4541-4549 awaits
run_owned(start_session_environment), and :4555-4556 then begins the fresh L timeout. start_session_environment at
provisioning/tensorlake_add.rs:357-390 performs validation, recipe/provider/place reads and awaits
add_confirmations.lock_owned at :388 before returning registration; that phase has no elapsed relay-budget subtraction.
agent_relay.rs:317-326 starts the answer timer on connection queue acceptance; :646-660 adds O+A+L. Thus the comment at
agent_requests.rs:4550-4554, 'answering by the end of the allowance after the Allow reaches the agent before its relay
gives up', is an unconditional timing explanation the source does not establish. At the same revision
SPEC_impl.md:9390-9396 expressly says the extra O is 'not a guaranteed ordering', and :6692-6698 says the local reply
'normally' wins. Those correct qualifications expose rather than refute the inaccurate new source comment. A path diff
from 190d844e to 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db leaves the timeout/comment lines unchanged (other path changes
concern unrelated deletion/test fields).

Relationship judgment: g020/l7 is useful direct evidence for the two phase origins, and a distinct documentation
assertion from f2. Qualifying that comment to normally would repair l7 while leaving SPEC.md's
shared-span-versus-fresh-allowance wording disagreement. Conversely, aligning the two specifications does not itself
remove the comment's unconditional ordering promise. Preserve both g005/f2 and g020/l7 source provenance and
consequences for parent consolidation; do not treat either as an automatic duplicate/drop or as a demand to alter the
accepted runtime timing policy. No new unrelated issue or source search is introduced by this synthesis.

Suggested bucket: other, specification/documentation accuracy. Confidence: confirmed written-model discrepancy and
distinct comment/phase-origin mismatch by source; no guaranteed-minimum-wait, production-ordering bug or lost-reply
failure alleged. proposed_drop: false (unchanged). possible_cover: caller-supplied g020/l7 is related late evidence and
an independently located guarantee claim, not established exact coverage of f2. Could-be-highest: false; both expiry
paths retain owned work and report unknown outcome; no security or loss-of-work route established. All readily checkable
named source premises resolved, no open lead. Source-only: no execution, test, build, reproduction, install, live action
or accepted parent drop/lowering.

Returned proposal (claims-002, event 7): **retain-verified**, suggested bucket **other**.

The specifications describe different effective deadline models. This is a bounded contract wording discrepancy, not a
minimum-response-time promise or an established lost reply/cancellation bug.

Original stronger consequence: Initial lead suspected agent/helm wait mismatch leaving a launch running after the caller
stopped waiting; subsequent source work refuted the missing-relay-budget implication. Retain only the established
effective-deadline contract discrepancy. No security, cancellation or work-loss route was alleged.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the two explicit conflicting deadline descriptions, keeping all corrections to the earlier
stronger timeout interpretation.

Existing parent disposition (event 7): verified. Parent explicitly accepted named retain-verified proposal from
claims-002 after inspecting it. The specifications describe different effective deadline models. This is a bounded
contract wording discrepancy, not a minimum-response-time promise or an established lost reply/cancellation bug. Retain
the two explicit conflicting deadline descriptions, keeping all corrections to the earlier stronger timeout
interpretation. Checked source/results: ["Read full b106c0e49646ecb9a17e1a06b17a70da0ece73e0 SPEC.md:4289-4341 and
SPEC_impl.md:6658-6702. Product wording uses a receipt-relative approval-plus-launch span and says slow approval leaves
less launch time. Implementation wording adds a relay span and a fresh thirty-minute post-Allow helm wait.","At
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, agent_requests.rs:4506-4557 waits for approval, registers owned work, then
times out result with ENVIRONMENT_LAUNCH_WAIT. This source confirms the post-Allow model as supplementary context; the
finding was introduced by the earlier documentation-only change.","Current SPEC.md:4293-4305 and SPEC_impl.md:6185-6213
retain the discrepancy. Neither timeout cancels the launch. Earlier minute-30/38-versus-39 examples are model
illustrations on a still-pending setup, not guaranteed minimum response times. No runtime failure was observed."]
Current coverage/results: ["The current authoritative specifications retain both descriptions; accepted background
continuation and relay expiry do not explain away the differing timeout origins or the 'slow answer leaves less time'
statement.","TODO.md Planned is empty. Launch deadline/approval budget/thirty-minute/timeout-origin searches of
plans/queue, BUGS.md, INDEX.md and active TRIAGE_OUTCOMES.md found no exact cover or same-root queued item.
environment-agent-operations is landed, not future cover.","FILTER.md does not cover persistent conflicting design
contracts. The claim does not depend on a rare runtime trigger or only an imprecise error message."] Exact cited cover:
[] Original stronger consequence and missing premise remain preserved: "Initial lead suspected agent/helm wait mismatch
leaving a launch running after the caller stopped waiting; subsequent source work refuted the missing-relay-budget
implication. Retain only the established effective-deadline contract discrepancy. No security, cancellation or work-loss
route was alleged."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g005 record](records/review-through-00de0b6c/groups/g005.json). Record ID: `f2`.

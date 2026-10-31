# OMP readiness comment excludes a valid early Save

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An OMP readiness comment says saving requires the first reply, although the current integration can persist a newly
created conversation earlier.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The existing eager persistence path contradicts the categorical first-reply explanation. Public guides are qualified and
do not create another defect. This is a pre-existing comment invariant, without a requested behavior change.

Source locations: `crates/farhelm-proto/src/lib.rs:1569-1571`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Original owner/ID a073/d1; separate bounded benign discovery. Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d.
Confirmed narrow documentation defect: the shared Pi/OMP rationale says both save their conversation file only once it
holds the first reply and report a resumable file only then. OMP /new is an explicit exception in SPEC.md:2649-2650,
SPEC_impl.md:3593-3595, website/src/content/docs/docs/agents/omp.md:19 and the reporter asset comment at
assets/omp-conversation-v1.ts:19-26. The readiness API states normal timing (proto/lib.rs:1544-1558), the generic
lifecycle guide says right after a launch and usually (stop-restart-resume.mdx:24-30), and the support matrix links
agent-specific gotchas (agents/index.mdx:8-12). Therefore the categorical internal comment should be qualified; this
does not establish a defect in the normal-timing metadata or the public matrix, nor any production Resume refusal. The
ordinary status builder derives the offer from committed capture rather than timing at service/status.rs:367-382,425;
the routine test proto/lib.rs:5164-5243 checks available wording and neutral language, not vendor persistence. Suggested
bucket other; confidence confirmed specification/comment contradiction. proposed_drop none; no exact Planned item, queue
plan, BUGS entry, queue finding, ledger decision or filter match found in targeted coverage searches. This is
PRE-EXISTING at the exclusive base: da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c:crates/farhelm-proto/src/lib.rs:1560-1562
carries the same comment. It is newly identified in this discovery pass, not newly introduced by the reviewed commit
range. Suggested correction is comment-only: describe initial-launch normal timing while noting OMP saves /new eagerly.
No runtime or vendor action was taken or evidence authenticated. No security, process-loss or user-work-loss consequence
is claimed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a073/d1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

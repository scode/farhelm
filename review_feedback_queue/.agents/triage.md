# Triage review feedback

This file is the triage procedure: starting a triage session, preparing each queued item, asking the user for an
outcome, and recording it in `TRIAGE_OUTCOMES.md`. The priority buckets and the boundary between triage and execution
are in `review_feedback_queue/AGENTS.md` and govern everything here. It belongs to the "Triage" reading list in
`review_feedback_queue/AGENTS.md`.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Starting triage

"Triage review feedback" means decide what to do with individual items, WITH the user. It does not authorize fixes or PR
creation. Read `review_feedback_queue/AGENTS.md`, the index, root `TRIAGE_OUTCOMES.md` if it exists, and the `Planned`
bucket in `TODO.md`. Work through items in index order unless the user chooses another order, skipping items with a
recorded decision unless asked to revisit them.

## Preparing each item

For each item, complete these steps before asking the user for an outcome:

1. Understand the feedback. Read the whole feedback file, trace the relevant current code paths, and read the
   authoritative spec sections. Identify the claimed behavior, the conditions that trigger it, and the consequence.
2. Verify whether the feedback is correct; do not take the reviewer's claim as established fact. Check its premises
   against the current code and specs, including guards or lifecycle rules that could invalidate it. Use a focused
   reproduction or check when code inspection cannot settle the claim, following the verification rules in `AGENTS.md`,
   "Tests and validation", `.agents/validation.md` and `.agents/test-evidence.md`. Distinguish a current problem from
   one that only existed at the reviewed commit. If verification is blocked or inconclusive, say exactly what remains
   unverified and why; do not present it as confirmed.
3. State the assessment to the user, with the evidence and any uncertainty: confirmed, partly correct, incorrect,
   already addressed, or unresolved. Keep correctness separate from whether the issue is worth addressing.

When verification shows a finding is already fixed on main, do not ask the user for an outcome: record it as `discard`
with the comment "already fixed" and the fixing commit or PR, remove its feedback file and index entry immediately, mark
execution complete, and move on. `discard` is used because it carries no judgment about the finding.

Before presenting an item for a decision, check whether its behavior is explicitly accepted by the current SPEC.md or
SPEC_impl.md, its fix is already covered by an item in TODO.md's `Planned` bucket, it is a known bug root `BUGS.md`
already records as one we have decided to live with, or it matches a review-only filter in
`review_feedback_queue/FILTER.md`. Verify the actual trigger, consequence, and scope against that acceptance, planned
work, known bug, or filter; sharing a subsystem or keyword is not enough. If fully covered, skip renewed discussion and
record `other` with the exact spec section, planned item, `BUGS.md` entry, or filter as the reason. Remove the feedback
file and its index entry immediately, and record execution as complete for that queue cleanup; do not leave the item to
be skipped again in later sessions. Already planned means acknowledged work, not an implemented fix. Do not implement
it, broaden its scope, or create another TODO merely because the same issue appears in feedback. If only part is
covered, bring the uncovered part to the user. Briefly report skipped items and their basis, then continue to the next
undecided item. This is an exception to the per-item decision question in `review_feedback_queue/.agents/triage.md`,
"Asking for a decision".

## Asking for a decision

Assume the user knows Farhelm as a tool but has read neither the feedback nor the relevant code. Begin every item with
plain-language context for the thing under discussion: name the user-facing feature or operation before describing any
internal details. If the finding does not map cleanly to a user-facing feature, name the architectural part of Farhelm
instead (for example, the helm's payload cache, the supervisor's session lifecycle, or the host registry) and explain
its role. Then state the reported bug explicitly in user-facing terms: what goes wrong, under what trigger, and what the
user observes or loses. Only after that context and bug statement, name the feedback filename and assess the finding's
correctness with its expected versus actual behavior and practical consequence. Give enough context to understand the
assessment without opening the file or knowing internal symbols. Prefer the user's visible behavior throughout. Do not
lead with terms such as “proven”, “unproven”, “refinement”, or other storage and implementation labels; translate them
into what the user sees first, and introduce an internal term only when it is needed to support the assessment.

Then recommend an outcome with its reason and ask the user to decide before recording an outcome or moving to the next
item. A recommendation is not a decision; unresolved items stay undecided. The outcomes are:

- `discard`: not worth the human's time at this time. This says nothing about the item's validity, severity, or merit.
  Drop it from the queue without remedial action; do not turn it into a spec exception, code fix, or TODO.
- `fix spec`: clarify the underlying principle in `SPEC.md` and/or `SPEC_impl.md` so similar feedback is suppressed for
  the right reason, not by blacklisting a particular finding. For example, an agreed target of roughly 100 sessions can
  make million-session scalability concerns out of scope. That example is not itself a new product requirement.
- `fix code`: the feedback is legitimate; address the problem within the existing specification.
- `fix spec+code`: make both the agreed specification change and the corresponding code change.
- `other`: negotiate a concrete custom outcome with the user, including what to do with the queue item.

## Recording the outcome

After each decision, create or update root `TRIAGE_OUTCOMES.md`. Use one heading per feedback filename, spelled exactly
as it appears in `review_feedback_queue/`, with these fields:

- Outcome: one of the five values in `review_feedback_queue/.agents/triage.md`, "Asking for a decision".
- Assessment: the correctness verdict, supporting evidence, and any unverified claims or limitations, separate from the
  user's chosen outcome.
- Decision: the user's rationale and agreed scope; preserve their meaning rather than substituting the recommendation.
  For spec changes, include the underlying principle; for `other`, include the negotiated action.
- Completion criteria: what execution must accomplish, including any deliberately retained part of the feedback.
- Execution: `pending` initially; later `in progress`, `blocked` with the reason, or `complete`, with the stable jj
  change ID, bookmark, and PR URL as they become available. `` planned in `plans/queue/<slug>.md` `` means the outcome
  was scheduled through `plans/` (see `plans/AGENTS.md`) and is executed only by running that plan.

Keep previous decisions and execution records; do not overwrite the file on a new triage session. If the user revises a
decision, retain the prior decision and note what supersedes it. Do not create entries for undecided items. Triaged
items are no longer awaiting a triage decision, but their feedback files and index entries stay until execution,
including for `discard`. This preserves the input for the separate execution step and its per-item PR. The automatic
spec-covered or already-planned cleanup in `review_feedback_queue/.agents/triage.md`, "Preparing each item", is the
exception: those queue items are removed during triage itself.

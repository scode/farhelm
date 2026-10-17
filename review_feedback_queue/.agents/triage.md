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

## Presenting items on one page

By default, undecided items are triaged on one page rather than one at a time in conversation. The maintainer chose this
on 2026-10-16. Prepare every item first ("Preparing each item" in this file, including the immediate removal of covered,
filtered and already-fixed items), then publish one `scode-snippets` snippet holding every item still undecided, and
give the maintainer its link. Walk through items one at a time in conversation only when the maintainer asks for it.

The page groups items by topic and keeps index order within a group. Each item says, in this order, what goes in a
conversation under "Asking for a decision" in this file: its number, bucket and feedback filename; a short
plain-language title; what the feature is; what goes wrong for the user; the assessment with its evidence and what
remains unverified; the bucket and why; and the recommendation with its reason. Then comes the decision question, with
one radio option per plausible outcome. Each option has a short label, its outcome value (`discard`, `fix spec`,
`fix code`, `fix spec+code`, `other`), and a line on what it would change and roughly how much work it is. The
recommended option comes first and is marked, a "leave for later" option comes last, and a free-text comment field
follows. Picks and comments survive a reload (per-browser storage is enough). A button at the bottom copies every
answered item as one `triage-answers` block of text, giving each item's filename, its pick (outcome and label) and its
comment; items with neither are left out. The `farhelm-triage-judgement-2026-10-15` and
`farhelm-triage-judgement-2026-10-16` snippets are working examples of the layout and the copy format.

Act on the pasted block item by item:

- a pick is the maintainer's decision, recorded as "Recording the outcome" in this file describes; a comment alongside
  it is their rationale and scope, preserved in the Decision field in their words, and an instruction in it (a spec
  principle to add, a wider rule to apply) becomes part of the completion criteria;
- a comment without a pick is a question or a proposal: answer it in conversation, and record nothing for that item
  until the maintainer decides;
- "leave for later", or neither a pick nor a comment, leaves the item undecided in the queue. It is never a `discard`.

Ask in conversation only where a comment is unclear. The pasted block is the decision, so do not ask the maintainer to
confirm picks before recording them.

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

# Execute triage outcomes

This file is the procedure for executing the decisions triage recorded in `TRIAGE_OUTCOMES.md`: one draft PR per
outcome, in one linear stack. It starts only on a separate command that names it (`review_feedback_queue/AGENTS.md`,
"Triage and execution are separate"). It belongs to the "Executing triage outcomes" reading list in
`review_feedback_queue/AGENTS.md`.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

"Execute triage outcomes" is a separate user request. Read root `TRIAGE_OUTCOMES.md` and execute its pending decisions
(or the subset the user names); resume incomplete execution rather than making duplicate PRs. Outcomes marked
`` planned in `plans/queue/<slug>.md` `` belong to that plan: execute them only as part of running it. Scheduling
outcomes through `plans/` instead of executing them here happens only when the user explicitly asks for the planning
system; `plans/AGENTS.md` describes it. Do not silently triage undecided items or change an agreed outcome. If current
code or specs invalidate a decision, return that item to the user for clarification.

Load and follow the `jjstack` skill. Make one reviewable commit, stable bookmark, and draft PR per triaged outcome,
including `discard` and `other`, in a single linear stack. Use ledger order unless dependencies require another order;
state the order before starting. Base each PR on the preceding bookmark, with the bottom PR based on the chosen stack
base per jjstack. Do not combine items into one PR or split `fix spec+code` across PRs.

Each item's PR contains its agreed changes, the corresponding `TRIAGE_OUTCOMES.md` execution update, and removal of its
feedback file and index entry (or the agreed narrowing for a partial/custom outcome). A discard PR contains only that
queue removal and outcome bookkeeping, with no spec or code change. Retain completed ledger entries so another execution
request can distinguish completed work from pending work. Record the change ID and bookmark before creating the PR, then
add its URL to the same change and update that PR; do not make a separate bookkeeping PR.

Validate each change using the rules in `AGENTS.md`, "Tests and validation", and `.agents/validation.md`. Completion
means the agreed work is verified and its draft PR exists, not that it has merged. Report the item-to-PR mapping and any
blocked items. This workflow never marks PRs ready, enables auto-merge, or merges them; publishing or landing requires a
separate user request.

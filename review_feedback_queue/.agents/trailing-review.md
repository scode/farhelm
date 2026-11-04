# Trailing code review

This file is the trailing code review: what the request means, who reviews by default, and the procedure from pinning
the range to the review-result PR. It belongs to the "Trailing code review" reading list in
`review_feedback_queue/AGENTS.md`.

## Trigger and default reviewer

"Perform trailing code review", or similar wording around "trailing code review", means adversarially review every
commit after the last trailing-reviewed commit through the current tip of `main`, then feed the findings into this
queue's existing recording flow. The saved checkpoint is in `review_feedback_queue/TRAILING_REVIEW.md`; the procedure is
`review_feedback_queue/.agents/trailing-review.md`, "Procedure".

Use the `persistent-review` skill for the primary review of the pinned range, with an adversarial charter emphasizing
security and correctness. Only the primary review runs under the skill; the independent drop check
(`review_feedback_queue/.agents/recording.md`, "Independent drop check") does not. Default to the current agent's
backend, model and reasoning effort. Honor any requested backend, agents, models, effort or review emphasis, subject to
the skill's supported backends and observable-delegation requirements. Do not ask the user to choose a reviewer when
they have not specified one. If the requested or default configuration cannot meet the skill's requirements, report the
limitation and leave the review incomplete rather than silently substituting.

## Procedure

`review_feedback_queue/.agents/trailing-review.md`, "Trigger and default reviewer", defines the trigger and default
reviewer. A trailing review covers `last-reviewed..review-tip`: the saved commit is excluded, and the pinned tip of
`main` is included. Requesting this review authorizes recording findings and updating the checkpoint; it does not start
triage, fixes, or execution of triage outcomes.

1. Fetch `origin/main` and pin its full commit hash as `review-tip`. Read `TRAILING_REVIEW.md` from that commit so a
   stale checkout cannot supply an older checkpoint. Verify that the saved full hash exists and is an ancestor of
   `review-tip`. If the checkpoint is missing or invalid, or the ancestry check fails, report the problem without
   inventing a replacement. If the two commits are equal, report that there is nothing new to review.
2. Use `persistent-review` to pin the required inputs and divide them into bounded functional groups, mapping every
   commit's changes to those groups. Include the surrounding code and specification context needed to judge them. A net
   diff between the endpoints is not sufficient: it can hide changes introduced and later reverted within the range. Use
   the pinned revisions rather than an unrelated or dirty working tree. Give each reviewer the two full hashes, the
   complete commit list, its assigned scope, and the instructions in `review_feedback_queue/.agents/recording.md`,
   "Running an agent review that feeds this queue". Also tell each reviewer to record every proposed drop of a finding
   in its findings log, whatever bucket it suggests, with the proposed basis and whatever text it cites; a drop
   mentioned only in prose is not recorded. The skill's findings log and progress files are required and stay in the
   agent scratch space; they are not the report files the queue's reviewer instructions forbid.
3. Run the groups under `persistent-review`, including concurrent launches, immediate findings logs, delivery audits,
   same-session continuation and later-turn synthesis. Monitor them as described under "Monitoring reviewers" below.
   Keep the skill's manifests, logs and completion evidence in the session's agent scratch space. Each reviewer's
   findings log is the authoritative record of its findings, candidates and leads, including `proposed_drop`,
   `possible_cover`, bucket and confidence. Tell reviewers that a continuation reply reports only what changed since
   their previous reply, and that they do not write out the three-part record in
   `review_feedback_queue/.agents/recording.md`, "Running an agent review that feeds this queue". After a reviewer's
   last turn, you render that record once from its findings log with the skill's tooling. The delivery audit reports
   supply the files and ranges read within the assigned scope; anything the reviewer itself reported as unread comes
   from its log. Every lead still open at the end is resolved into a finding or a refuted candidate, or recorded as a
   finding that keeps its uncertainty. Reconcile the logs, including withdrawn findings, with the replies so candidates
   cannot disappear before the queue's independent drop check. A drop that appears only in a reply's prose goes back to
   its reviewer to be recorded in its log: in its existing synthesis or discovery turn, or, when its turns are over, in
   one continuation that asks only for that and starts no new discovery. Apply the usual coverage, deduplication and
   filtering requirements. Then build the check's register: every withdrawn finding, every finding with `proposed_drop`
   and every finding whose latest event lowers its consequence gets an entry regardless of bucket (do not filter this
   register by bucket), along with every drop you proposed while applying those requirements. Run the check as
   `review_feedback_queue/.agents/recording.md`, "Independent drop check", describes, outside `persistent-review`.
   Record retained findings in feedback files and `INDEX.md`, preserving their reviewed commits and provenance. A
   trailing review is an automated review of committed code for the purposes of `FILTER.md`.
4. Only after every group meets `persistent-review`'s completion requirements, every commit in the range has been
   reviewed, every finding and proposed drop has been handled, and all retained findings have been recorded, update
   `TRAILING_REVIEW.md` to `review-tip` in the same change as the queue updates. Findings need not be fixed for the
   review to be complete. An incomplete persistent review, unread scope, or an unresolved recording step or independent
   drop check leaves the checkpoint unchanged; retain findings already produced and report the gap. Do not advance to
   commits that landed after the pinned tip, and do not overwrite a checkpoint another run has advanced without
   reconciling its coverage and queue changes. Besides its heading and preamble, `TRAILING_REVIEW.md` holds only the
   `Last reviewed commit:` line, whose format step 1 relies on, and the latest run's completion record. Before writing
   this run's record there, move the record already there to the top of
   `review_feedback_queue/TRAILING_REVIEW_HISTORY.md`, below its NOTE. The history file is not read during a review.
   When rebasing over another run's review result, reconcile the history file so that each record appears in it once.
5. Use `jjstack` to put the checkpoint update, the move into the history file and any queue updates in one review-result
   commit and draft PR, including when there are no retained findings. The checkpoint on `main` advances when that PR is
   merged; preparing the result alone does not advance it there. Merging is a separate manual step for the maintainer.
   Do not merge automatically.

Report the exact range reviewed, any coverage gaps, the findings recorded or extended, and whether the checkpoint update
is pending in a draft PR or has reached `main`. Include the compact completion record required by `persistent-review`,
with references to the retained evidence.

Close the findings report with the total number of new queue items and a summary grouped by priority bucket, in order:
`highest`, `high`, `other`. Show each bucket's count, including zero, and give a short user-facing TLDR for every new
item under its bucket. Count only newly recorded items; describe extensions to existing items separately before this
summary. Preserve material uncertainty in each TLDR so a conditional lead does not read as a confirmed bug.

Put the review-result draft PR's diff link immediately after the grouped summary. If no draft PR was created, put that
fact and the reason there instead; do not imply that an interrupted review completed or create a PR when there were no
new commits to review. The closing status banner below, when required, comes after the link or explanation.

## Monitoring reviewers

Monitor every running reviewer for stalled progress and blocked turns, including cybersecurity refusals. Start with job
state and exit status, file timestamps and growth, compact delivery audits, findings counts and short progress notes.
Silence alone does not establish a stall. Use the persistent-review and shellout skills' monitoring and continuation
rules; inspect findings and final replies when they need a decision.

Keep whole transcripts and routine activity out of the orchestrator's context. Delivery audits use the evidence helper,
not manual transcript reading. When metadata cannot explain a suspected stall, use a separate diagnostic agent with only
the job handle and artifact paths. Have it inspect execution metadata and narrowly relevant errors or refusal text, then
return the failure category, last observed progress and next permitted step. It must not read implementation payloads,
reproduce a bug, perform the review, or return source or transcript dumps. This diagnosis earns no review coverage.
Targeted transcript inspection still needs the permission required by `persistent-review`.

If a reviewer encounters a cybersecurity block, continue its original identity only with permitted defensive work.
Narrow the instructions as needed: source inspection and documented contracts, with no bug reproduction, exploit or
payload construction, runtime or network experiments, process operations, or live-install actions. Do not use a helper,
another identity, concealed summaries or repeated requests to bypass a safety control. A narrower continuation does not
establish coverage of work it omits. If the reviewer still cannot complete its assigned scope, preserve its findings and
evidence, report the exact gap, and leave the checkpoint unchanged under the completion rule above.

## Closing status banner

Trailing-review result and merge reports end with a loud Markdown banner only when the review is incomplete or
checkpoint handling is exceptional; an ordinary draft PR awaiting merge needs no banner.

Use the banner for unread scope, unmet review or recording requirements, a result that omits the checkpoint advance to
the pinned tip, a requested merge that fails to land that advance, or an explicitly authorized advance despite an
incomplete review. A completed review with its checkpoint proposal in the draft PR is the normal outcome. The marker
reaches main when the PR merges; report that publication state plainly without the banner. If the checkpoint already
equals the tip and there is nothing new to review, no banner is needed.

An investigated uncertainty retained under `persistent-review`'s completion rules does not itself make the review
incomplete or trigger the banner.

Use this standalone level-two heading, followed by bold status text, as the final block of the response:

```markdown
## TRAILING REVIEW NOT FULLY COMPLETE

**Review: <complete or incomplete, with any remaining gap>. Checkpoint: <the exception, actual saved hash on main and
any proposed or explicitly authorized advance>. Next review starts after <actual saved hash on main>.**
```

State the actual review and checkpoint status separately. An explicitly authorized checkpoint advance does not turn an
incomplete review into a completed one. Keep the banner outside code fences in the actual response, and put nothing
after it: no PR link, validation note, or sign-off. An ordinary sentence elsewhere in the response is not a substitute.

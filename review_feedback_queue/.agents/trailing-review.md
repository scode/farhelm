# Trailing code review

This file is the trailing code review: what the request means, who reviews by default, and the procedure from pinning
the range to the review-result PR. It belongs to the "Trailing code review" reading list in
`review_feedback_queue/AGENTS.md`.

## Trigger and default reviewer

"Perform trailing code review", or similar wording around "trailing code review", means adversarially review every
commit after the last trailing-reviewed commit through the current tip of `main`, then feed the findings into this
queue's existing recording flow. The saved checkpoint is in `review_feedback_queue/TRAILING_REVIEW.md`; the procedure is
`review_feedback_queue/.agents/trailing-review.md`, "Procedure".

Use the `persistent-review` skill for the reviews, with an adversarial charter emphasizing security and correctness.
Default to the current agent's backend, model and reasoning effort. Honor any requested backend, agents, models, effort
or review emphasis, subject to the skill's supported backends and observable-delegation requirements. Do not ask the
user to choose a reviewer when they have not specified one. If the requested or default configuration cannot meet the
skill's requirements, report the limitation and leave the review incomplete rather than silently substituting.

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
   "Running an agent review that feeds this queue".
3. Run the groups under `persistent-review`, including concurrent launches, immediate findings logs, delivery audits,
   same-session continuation and later-turn synthesis. Keep the skill's manifests, logs and completion evidence in the
   session's agent scratch space. Reviewers still return the queue's required findings and candidate fields in their
   final replies; the persistent logs supplement those replies. Reconcile the logs, including withdrawn findings, with
   the replies so candidates cannot disappear before the queue's independent-review checks. Apply the usual coverage,
   deduplication, filtering, and independent-review requirements before recording findings. Record retained findings in
   feedback files and `INDEX.md`, preserving their reviewed commits and provenance. A trailing review is an automated
   review of committed code for the purposes of `FILTER.md`.
4. Only after every group meets `persistent-review`'s completion requirements, every commit in the range has been
   reviewed, every finding and proposed drop has been handled, and all retained findings have been recorded, update
   `TRAILING_REVIEW.md` to `review-tip` in the same change as the queue updates. Findings need not be fixed for the
   review to be complete. An incomplete persistent review, unread scope, or an unresolved recording or
   independent-review step leaves the checkpoint unchanged; retain findings already produced and report the gap. Do not
   advance to commits that landed after the pinned tip, and do not overwrite a checkpoint another run has advanced
   without reconciling its coverage and queue changes.
5. Use `jjstack` to put the checkpoint update and any queue updates in one review-result commit and draft PR, including
   when there are no retained findings. The checkpoint on `main` advances when that PR is merged; preparing the result
   alone does not advance it there. Merging is a separate manual step for the maintainer. Do not merge automatically.

Report the exact range reviewed, any coverage gaps, the findings recorded or extended, and whether the checkpoint update
is pending in a draft PR or has reached `main`. Include the compact completion record required by `persistent-review`,
with references to the retained evidence.

End the final response with the total number of new queue items and a summary grouped by priority bucket, in order:
`highest`, `high`, `other`. Show each bucket's count, including zero, and give a short user-facing TLDR for every new
item under its bucket. Count only newly recorded items; describe extensions to existing items separately before this
summary. Preserve material uncertainty in each TLDR so a conditional lead does not read as a confirmed bug.

Put the review-result draft PR's diff link immediately after the grouped summary, as the final element of the response.
If no draft PR was created, end with that fact and the reason instead; do not imply that an interrupted review completed
or create a PR when there were no new commits to review.

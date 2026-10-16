# Trailing code review

This file is the trailing code review: what the request means, who reviews by default, and the procedure from pinning
the range to the review-result PR. It belongs to the "Trailing code review" reading list in
`review_feedback_queue/AGENTS.md`.

## Trigger and default reviewer

"Perform trailing code review", or similar wording around "trailing code review", means adversarially review every
commit after the last trailing-reviewed commit through the current tip of `main`, then feed the findings into this
queue's existing recording flow. The saved checkpoint is in `review_feedback_queue/TRAILING_REVIEW.md`; the procedure is
`review_feedback_queue/.agents/trailing-review.md`, "Procedure".

By default, use a separate adversarial review agent running the same model as the current agent, with an emphasis on
security and correctness. Honor any requested agents, models, or review emphasis. Do not ask the user to choose a
reviewer when they have not specified one.

## Procedure

`review_feedback_queue/.agents/trailing-review.md`, "Trigger and default reviewer", defines the trigger and default
reviewer. A trailing review covers `last-reviewed..review-tip`: the saved commit is excluded, and the pinned tip of
`main` is included. Requesting this review authorizes recording findings and updating the checkpoint; it does not start
triage, fixes, or execution of triage outcomes.

1. Fetch `origin/main` and pin its full commit hash as `review-tip`. Read `TRAILING_REVIEW.md` from that commit so a
   stale checkout cannot supply an older checkpoint. Verify that the saved full hash exists and is an ancestor of
   `review-tip`. If the checkpoint is missing or invalid, or the ancestry check fails, report the problem without
   inventing a replacement. If the two commits are equal, report that there is nothing new to review.
2. Give the reviewer the two full hashes, the complete commit list, and the instructions in
   `review_feedback_queue/.agents/recording.md`, "Running an agent review that feeds this queue". Review every commit's
   changes and the surrounding code and specification context needed to judge them. A net diff between the endpoints is
   not sufficient: it can hide changes introduced and later reverted within the range. Use the pinned revisions rather
   than an unrelated or dirty working tree.
3. Collect coverage and findings using the existing agent-review rules. Apply the usual coverage, deduplication,
   filtering, and independent-review requirements before recording findings. Record retained findings in feedback files
   and `INDEX.md`, preserving their reviewed commits and provenance. A trailing review is an automated review of
   committed code for the purposes of `FILTER.md`.
4. Only after every commit in the range has been reviewed, every finding and proposed drop has been handled, and all
   retained findings have been recorded, update `TRAILING_REVIEW.md` to `review-tip` in the same change as the queue
   updates. Findings need not be fixed for the review to be complete. An interrupted review, unread scope, or an
   unresolved recording or independent-review step leaves the checkpoint unchanged; report the gap. Do not advance to
   commits that landed after the pinned tip, and do not overwrite a checkpoint another run has advanced without
   reconciling its coverage and queue changes.
5. Use `jjstack` to put the checkpoint update and any queue updates in one review-result commit and draft PR, including
   when there are no retained findings. The checkpoint on `main` advances when that PR is merged; preparing the result
   alone does not advance it there. Merging is a separate manual step for the maintainer. Do not merge automatically.

Report the exact range reviewed, any coverage gaps, the findings recorded or extended, and whether the checkpoint update
is pending in a draft PR or has reached `main`.

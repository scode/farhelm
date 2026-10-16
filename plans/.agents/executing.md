# Picking and executing a plan

How an executor picks a plan, claims it, runs it, blocks, delivers, gives a claim back and closes, with the step numbers
every other file cites. Executors and drainers read all of it before step 1 of `plans/.agents/executing.md`, "Executing
one plan", since steps 9, 10 and 12 and the paragraphs after step 12 govern work that starts at step 6. The planner
reads it to write goal files that match it, and the landing monitor reads it for the shared bookkeeping files listed
under `plans/.agents/executing.md`, "Picking: "pick a plan to execute", "execute the next plan"", and the cold read of
step 10.

## Picking: "pick a plan to execute", "execute the next plan"

Those requests, and "use the planning system and pick one item to execute", mean: pick one plan by the judgment in this
section, then execute it. "Execute the next plan in order" takes the first eligible plan regardless of conflicts.
"Execute plan X" takes X if it is eligible and otherwise says why (in flight, blocked, waiting on a dependency) and
stops; it does not wait.

A plan is eligible when it is `pending` and every plan it runs after is gone from `INDEX.md`.

Unlanded work is every listed plan that is in flight, or that has open `plan/<slug>/*` PRs whatever its state (including
a pending plan that has PRs from an earlier round). When judging a candidate, its own PRs do not count.

The judgment: take the oldest eligible plan unless it is particularly likely to conflict with unlanded work. That means
it would change the same functions, the same spec sections, or substantially the same parts of a file. Touching the same
crate or the same file is not enough on its own, and these shared bookkeeping files never count: `TODO.md`,
`TRIAGE_OUTCOMES.md`, `review_feedback_queue/` (its index and the feedback files PRs delete), `releasing/changelog.d/`,
and `plans/`. A skipped plan is not overtaken on the same ground: a newer candidate that would conflict with an older
eligible plan already skipped for conflicts is skipped too, or the older one could wait forever. If every eligible plan
is skipped, take nothing.

To judge, read `queue status`, the candidates' plan files, and for unlanded work its plan file and the `gh pr diff` of
its open PRs. Record the pick and its reasoning in the executor log.

## Executing one plan

1. Read the executor log. If it records a claim, `jj git fetch` and run `queue status` (and `queue history <slug>` where
   the line alone does not settle it), then go by the verb the log says you were running:
   - Nothing pending (you were working on the plan): if the line carries your claim id, resume at step 5; otherwise the
     claim was released while you were away.
   - `claiming`: your id on the line means the claim landed; resume at step 5. Otherwise it never landed: clear the log
     entry and pick afresh, with no notification.
   - `unclaiming`: your id still on the line means the `unclaim` never landed; run it again. Once it has landed, finish
     the close it was part of: step 12 if the plan is still unclaimed, or, if another executor has claimed it since, the
     reset for a released claim (`plans/.agents/close-reset.md`, "Clean-main boundary"), recorded in the executor log
     only.
   - `blocking` or `delivering`: if `history` shows that commit with your claim id, it landed, whatever the line says
     now (the monitor or a review may already have moved on); finish closing (step 12). For `blocking`, also notify: the
     crash may have come between the block and its notification, and a second notification costs less than none. If not
     and your id is still on the line, run the verb again. Otherwise the claim was released.

   A released claim means: drop it from the executor log, push nothing more for that plan and write nothing more to its
   working log, run the reset for a released claim (`plans/.agents/close-reset.md`, "Clean-main boundary"), and continue
   with a fresh pick. A release is the maintainer's own doing, so it is not worth a notification.
2. Apply the clean-main boundary, then run `queue status` and pick (`plans/.agents/executing.md`, "Picking: "pick a plan
   to execute", "execute the next plan""). Nothing picked: the round is over.
3. Claim. Generate a claim id (`python3 -c 'import secrets; print(secrets.token_hex(3))'`), write slug, id and
   `claiming` to the executor log, then `queue claim <slug> --claim <id>`. Exit 10 means someone else got there first:
   pick again, reusing the analysis you already have for the remaining candidates. Exit 3 is ambiguous: settle it before
   claiming anything else, by the exit-3 rule under `plans/.agents/queue.md`, "States, and the script that moves them".
   Run `queue status`; if the line carries your id, the claim landed. If it still shows `pending`, run the same `claim`
   again with the same id and settle that result the same way. If it shows someone else's claim, the plan is theirs:
   clear the entry from the executor log and pick again. Never move on with a claim still uncertain: the executor log
   records one claim, so a second pick would lose track of the first, and a first claim that landed late would then sit
   in flight with nobody working on it.
4. Re-check. Run `queue status` again. If a plan claimed since your pick is a strong conflict with this one, give the
   claim back (`queue unclaim`) and pick again from step 2, whose boundary brings the checkout up to the `main` the
   unclaim just moved.
5. Run `jj git fetch`, then read the plan file from the freshly fetched `main@origin`, including any `## Decisions`. The
   fetch in step 2 came before the pick and the claim, so the local `main@origin` can predate a revision or an answer to
   this plan that landed in between; the claim itself goes through GitHub and does not update it.
6. Set up the base from the clean `main@origin` working copy. If the plan has open `plan/<slug>/*` PRs (a resume), track
   those bookmarks, rebase that stack onto `main@origin` as a careful rebase (`.agents/careful-rebase.md`), and build on
   its tip; otherwise start a new change on `main@origin`. Either way, mark the start of the plan's builds for the prune
   at close (`plans/.agents/close-reset.md`, "Pruning build leftovers"). Conflicts in the shared bookkeeping files
   listed under `plans/.agents/executing.md`, "Picking: "pick a plan to execute", "execute the next plan"" are
   mechanical: keep both sides. A conflict that needs a design decision is a block (step 10). A plan PR the maintainer
   closed without merging is the maintainer rejecting it, unless a Decisions entry says otherwise: block with that as
   the question rather than rebuilding it or carrying on above it.
7. Resume check, whenever the plan has a working log or open PRs. Read the whole working log, the open PRs, and the
   Decisions, then write a "where this stands" entry in the working log: what is built and pushed, what is half done,
   what the latest decision asks for, and what comes next. If the log names another checkout and its last recorded
   pushes differ from GitHub's, inspect that checkout without changing it (`jj --ignore-working-copy -R <path> log`);
   unpushed work found there is a block, with that as the question, rather than something to rebuild blind. A sub-agent
   then checks the entry against the same sources. Reconcile any disagreement before working. If the plan cannot be
   reconciled, give the claim back (the paragraph after step 12) and notify. Always start your own resource watchdog;
   treat the previous executor's scratch directory as read-only input. After a release, entries the released executor
   wrote to the working log before it noticed are expected: record that you saw them and continue, rather than stopping
   as the resume protocol would for entries of unknown origin.
8. Run the plan file as the goal. For a single plan, set the harness's goal to it if the harness lets the model set its
   own goal; otherwise, and always within a drain, behave exactly as if it were the harness-provided goal under the
   outer request. A drain does not register each plan as a harness goal, because a plan that blocks could not be cleared
   to make room for the next one.
9. Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id. If it does
   not, stop pushing and treat the claim as released (step 1). After each push, record the pushed commits in the working
   log.
10. Blocking: when the remaining work waits on a decision only the maintainer can make (the goal file's unattended
    fallback), push everything that is consistent, then write the question for the Blocked section
    (`plans/.agents/maintainer-writing.md`, "Writing for the maintainer"). Have a sub-agent cold-read it, asking whether
    the maintainer could decide from this text alone and whether it breaks public-repo hygiene, and revise until it
    passes. Write `blocking` to the executor log, run `queue block <slug> --claim <id> --question <file>`, notify
    (`plans/AGENTS.md`, "Notifications"), and close (step 12).
11. Finishing: when the done criterion holds, rebase the stack onto `main@origin` as a careful rebase if main has moved
    in a way that touches it, so the report describes what would actually land. Then write the report
    (`plans/.agents/maintainer-writing.md`, "Writing for the maintainer") and have a sub-agent cold-read it, asking
    whether the maintainer could approve or ask for a follow-up from it alone and whether it breaks public-repo hygiene.
    Revise until it passes, write `delivering` to the executor log, and run
    `queue deliver <slug> --claim <id> --report <file>`. The plan is now `[complete]`, and the monitor lands it from
    there.
12. Close: return to clean `main` with the reset under `plans/.agents/close-reset.md`, "Clean-main boundary", prune
    build leftovers (`plans/.agents/close-reset.md`, "Pruning build leftovers"), write a closing entry in the plan's
    working log that names any paths the reset discarded, stop its resource watchdog, and clear the claim from the
    executor log. Closing ends the plan's resume-log protocol and its review demands, as an express stop; the next plan
    starts its own.

`block` or `deliver` exiting 10 means the claim was released while you worked: push nothing more, record it in both
logs, run the reset for a released claim (`plans/.agents/close-reset.md`, "Clean-main boundary"), and prune build
leftovers. To give a claim back for any other reason (the maintainer stops the flow mid-plan, a `gh` login that expired,
a disk alert that cannot be cleared), push what is consistent, write the working log, write `unclaiming` to the executor
log, run `queue unclaim`, and close (step 12). Notify unless the maintainer asked for the stop: an expired login or a
full disk stops every plan on the machine until someone fixes it, and a plan that could not be reconciled (step 7) will
stop the next executor the same way. If pushing the consistent work failed (the expired login, say), close with the
reset for a released claim instead of the full one, so that work stays in the checkout for the next claimer to find
(step 7), and say so in the working log and the notification.

To notify, follow `plans/AGENTS.md`, "Notifications". Delivering does not notify: the monitor lands the plan, and its
report reaches the maintainer through `REPORTS.md`. An executor that dies cannot notify at all; its claim shows up in
`queue status` as possibly abandoned, and the monitor notifies about that (`plans/.agents/landing.md`, "A landing
round").

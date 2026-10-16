# Landing

The landing monitor: starting and resuming, a landing round with its cross-plan review, blocking or stopping a landing,
and waiting between rounds. The landing monitor reads it before reading its log, and again at every round boundary
(`plans/AGENTS.md`, "Flows and reading lists"). The release plan X flow cites its flags.

## Landing: "monitor for complete plans", "land the complete plans"

NOTE: Landing does not wait for the maintainer. Executors deliver finished plans as `[complete]`, the monitor lands
them, and the maintainer reads the reports afterwards (`plans/.agents/reviewing.md`, "Reviewing the plans: "review the
plans""). That makes the monitor's own review of what it lands the last check before main, so it is done on purpose, not
by letting version control report conflicts.

"Monitor for complete plans" runs landing rounds until the maintainer says stop, waiting in between. "Land the complete
plans" is one round, then stop. Run either in its own checkout, and only one at a time: landing claims keep two monitors
from landing the same plan, but each would review what it is about to land without seeing what the other lands.

### Starting and resuming

Read the monitor log first (`plans/.agents/outside-files.md`, "Files outside the repository"). Check for the ntfy topic
file (`plans/AGENTS.md`, "Notifications"), unless the log already records the maintainer's choice to run without
notifications. If the log records a landing claim, `jj git fetch` and run `queue status` and `queue history <slug>`,
then go by the verb the log says you were running:

- Nothing pending (you were landing it): if the line carries your claim id, carry on with that plan from step 2 of the
  round in `plans/.agents/landing.md`, "A landing round", after the cross-plan review that opens the round (main may
  have moved while the monitor was down). Read which of its PRs GitHub shows merged rather than trusting the log's list,
  since a merge may have happened after the log was last written. If the line no longer carries your id, the maintainer
  released the claim: clear it and leave the plan to the next round.
- `starting`: your id on the line means the claim landed; carry on as in the first item of this list. Otherwise it never
  landed: clear it.
- `blocking`, `stopping` or `recording`: if `history` shows that commit with your claim id, it landed; clear the claim
  from the log, and for `blocking` or `stopping` notify, since the crash may have come before the notification went out.
  If not and your id is still on the line, run the verb again. Otherwise the maintainer released the claim: clear it and
  leave the plan alone.

A dirty working copy found here belongs to the recorded landing (a rebase or a fix in progress): carry on with it when
the landing carries on, and reset it the way an executor's close does (`plans/.agents/close-reset.md`, "Clean-main
boundary") when the claim was cleared. Only a dirty working copy with no recorded claim aborts the start, as the
clean-main boundary says.

Then write `running` to the Status section and start a round.

### A landing round

Apply the clean-main boundary, then run `queue status` and record the main commit id it prints. Look at its flags:

- `possibly abandoned` on an executor's claim: the plan stays stuck until the maintainer releases it, and a dead
  executor cannot notify for itself, so notify.
- `complete, no open PRs, merged ...`: a landing merged the whole stack and stopped before `landed`, and its claim was
  released. Finish it in this round: claim it with `start-landing`, confirm from GitHub that the merged PRs are the
  plan's whole stack (the report's PR list), and run `landed` with those numbers and notes that say the landing was
  finished after a released claim. Nothing else about it needs doing, and its dependents wait until it is recorded.
- `complete, but no open PRs and none found merged`: the plan's PRs were closed, and nothing moves until the maintainer
  decides what happens to it; notify, and leave it. Search can lag a fresh merge by a minute, so look again on the next
  round before believing it.
- `landing, no open PRs` on a landing claim that is not yours: another monitor or an interrupted one, which only the
  maintainer can sort out; notify and leave it.

These flags stay up across rounds, so notify for each once (per claim id, or per plan for the second), and keep what has
been notified in the monitor log.

If no plan is `[complete]`, the round is over.

NOTE: A round's plans are fixed when it starts: the plans this `queue status` shows as `[complete]`, and no others. Land
every one of them (merged, or blocked, or given back) before claiming anything else. A plan that completes while the
round runs belongs to the next round, which starts as soon as this one ends (`plans/.agents/landing.md`, "Waiting
between rounds"), however small it looks and however much it overlaps the plans in flight. Every executor builds and
tests on main, so each hour a finished plan waits unmerged is an hour of other work on a staler main than it could have.
Do not:

- claim (`start-landing`) a plan that completed after the round started, even to "review it in parallel";
- add such a plan to the round's stack, or restack the round to fit it in;
- restart a round's combined checks so that they also cover a newly completed plan;
- hold back a plan whose checks have passed so that it can merge together with plans that are still being checked;
- treat a watcher wake-up during a round as a reason to change the round. Note the newly completed plans in the monitor
  log and leave them `[complete]`.

On 2026-10-10 a round that started with three plans grew to nine this way, one claim at a time, and nothing merged for
about two hours.

Before anything merges, review on purpose what has landed on main since each complete plan's stack was based, and what
is about to land together: the diffs, not the titles, read against each other. Look for interactions that produce no
textual conflict: something one change renamed or repurposed that another relies on, a contract a new caller assumes
that another change altered, a new caller of changed code, a changed invariant, lock or ordering, a SPEC.md or
SPEC_impl.md amendment that another change now contradicts. `.agents/careful-rebase.md` lists the kinds; apply them
across the whole set, not one stack at a time. Have a fresh-context sub-agent do the same review independently and
reconcile any disagreement. Fix what is obvious as part of the landing; a conflict that needs a design decision blocks
the plan it concerns (the paragraph after the numbered list in this section), and the round goes on with the others.

Then, for each `[complete]` plan in queue order:

1. Claim it. Generate a claim id (`python3 -c 'import secrets; print(secrets.token_hex(3))'`), write slug, id and
   `starting` to the monitor log, then `queue start-landing <slug> --claim <id>`. Exit 10 means it is no longer complete
   (the maintainer abandoned or released it): skip it. Exit 3 follows the rule under `plans/.agents/queue.md`, "States,
   and the script that moves them".
2. Fetch, and rebase its stack onto `main@origin` as a careful rebase. Conflicts in the shared bookkeeping files listed
   under `plans/.agents/executing.md`, "Picking: "pick a plan to execute", "execute the next plan"" are mechanical: keep
   both sides. Main has moved since the review before step 1 if an earlier plan just landed; check what that plan
   changed against this one again.
3. Confirm no PR in the stack touches `plans/`.
4. Run the checks the rebase calls for, per `AGENTS.md`, "Tests and validation", and `.agents/validation.md`.
5. Land the stack bottom-up per `jjstack`, marking each PR ready as it lands, and record each merged PR in the monitor
   log. Before each merge, run `queue status` and confirm the line still carries your claim id; if it does not, the
   maintainer released it, so stop landing that plan and treat it as released (`plans/.agents/landing.md`, "Starting and
   resuming"). Bookkeeping commits make main move often, so a merge refused because the base branch changed means
   re-read and retry, not stop.
6. Write the landing notes. They become the report's `### Landing` section, which the maintainer reads after the fact,
   so follow `plans/.agents/maintainer-writing.md`, "Writing for the maintainer", and `plans/AGENTS.md`, "Public-repo
   hygiene", and use `####` or deeper headings if any. Say what else landed in the same round or since the stack was
   based and whether any of it interacted with this plan, which fixes the landing made and why, the checks run, reused
   and skipped (as redacted commands and recorder run ids, per `.agents/test-evidence.md`), and anything in the
   executor's report that the landing made untrue.
7. Write `recording` to the monitor log, then
   `queue landed <slug> --claim <id> --merged <every PR number that landed> --notes <file>`, which refuses while any of
   the plan's PRs is still open and checks that each named PR merged. Clear the claim from the monitor log, and return
   to a clean `main` (`jj new main@origin`).

When a landing needs a decision only the maintainer can make (a conflict that needs a design choice, checks that fail
without an obvious fix, a PR closed without merging), push nothing that is not consistent, then write the question per
`plans/.agents/maintainer-writing.md`, "Writing for the maintainer", naming which of the plan's PRs already merged and
what main now holds of the plan. Have a sub-agent cold-read it as in `plans/.agents/executing.md`, "Executing one plan",
step 10, write `blocking` to the monitor log, run `queue block <slug> --claim <id> --question <file>`, notify, return to
a clean `main`, and go on with the next plan.

A failure that is not a decision (an expired GitHub login, a disk that is full) is the monitor's own problem: write
`stopping`, run `queue stop-landing <slug> --claim <id>` if it can still reach GitHub, notify with the reason, write
`stopped` with the reason to the Status section, and stop. A plan left in `[landing]` because even that failed shows up
in `queue status` for whoever looks next, and `release plan X` returns it to `[complete]`.

Validation during a landing never ends on a clock. Do not pass the recorder's `--timeout`, wrap a command in `timeout`,
or give any other hard-coded deadline the power to kill a test run or a build. A deadline picked before the run starts
is a guess about how slow the machine and the failures will be, and a run killed at that minute reports everything it
had not reached as "did not run", which is evidence of nothing. Start a long run in the background with nothing that
ends it on a timer. If you want to look at it after a while, arm something that only wakes you: a watcher on the run's
log, or the harness's own scheduled wake-up. On waking, investigate: read the progress and the failures so far, then
keep waiting, start a narrower run, or stop this one on purpose because what it is still doing no longer helps. Stopping
a run is that decision, made with the evidence in hand, never a timer's. Time limits that belong to a suite itself, such
as Playwright's per-test timeout or the nextest configuration's slow-test settings, are unaffected. Where the harness
caps how long a background command may live, use the longest cap it allows and wake well before it, so the cap is never
what ends the run.

### Waiting between rounds

"Monitor for complete plans" waits the way a monitoring executor does (`plans/.agents/watching.md`, "Drain and keep
monitoring"), with the same watcher run the same way, except for its role, what its exits mean, and a `--max-wait 6600`
that makes it wake about every 110 minutes even when nothing changed. The executors have no use for that wake-up, but
the monitor does: it is how a claim that went quiet gets noticed, and how the review flow can tell the monitor is still
alive.

- Start
  `scripts/plans-watch.sh --repo <OWNER/NAME> --baseline-from <commit> --wake-check <checkout>/scripts/plans-queue.py --wake-for lander --max-wait 6600`,
  where `<commit>` is the main commit id printed by a `queue status` run when the round has finished, not the one from
  its start. If that status still shows a `[complete]` plan with open PRs (one whose landing claim was released during
  the round, or one delivered while it ran), run another round instead of waiting: the watcher only wakes for what
  changes after its baseline, so anything already complete there would wait for an unrelated change. With
  `--wake-for lander` it exits only when a plan is `[complete]` with a report it did not have at that commit: newly
  delivered, delivered again after a follow-up, or given back to `[complete]` by a release. Record its task handle and
  the commit in the monitor log.
- `changed` (status 0): run a round.
- `idle` (status 10): run `queue status`. If it shows a `[complete]` plan with open PRs, run a round: the watcher should
  have woken for it, and this catches any way it did not. Otherwise act on its flags only (as at the start of a round),
  then start the watcher again with the same commit. This idle wake, about every 110 minutes, is also how a claim that
  went quiet gets noticed.
- `error:` (status 3): if the line mentions HTTP 401 or 403, the GitHub login needs the maintainer; notify, write
  `stopped` with the reason, and stop. Otherwise keep restarting the watcher with `--interval 1800`, still with
  `--max-wait 6600`, until one exits `changed` or `idle`, which also restores the default interval; an outage still
  being retried is not notified.
- `usage:` (status 2) or anything else: notify, write `stopped` with the reason, and stop.

Rewrite the Status section on every wake, at the end of every round, when stopping, and whenever you check on a long
validation run, so a landing that takes hours does not look like a dead monitor; when the maintainer says stop, write
`stopped <time>: stopped by the maintainer`. "Land the complete plans" writes `stopped <time>: one round finished` when
its round ends, since no watcher follows it. The monitor notifies only for the conditions this section names (a landing
it blocked, a landing it could not finish, the three `queue status` flags, and monitoring that stopped for any reason
other than the maintainer), each of which can stop progress until the maintainer acts. A landing that went through is
not worth a notification; it shows up in `REPORTS.md` for the next review.

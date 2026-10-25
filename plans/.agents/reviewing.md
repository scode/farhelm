# Reviewing

Reviewing landed plans and answering blocked ones, including abandoning a plan. The plan reviewer reads it before
reading the monitor's status (`plans/AGENTS.md`, "Flows and reading lists").

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Reviewing the plans: "review the plans"

NOTE: This is review after the fact. By the time a plan reaches this flow its work is already on main; approving it ends
its tracking, and it does not gate anything. The one exception is a blocked plan, which waits for an answer.

Do its work in a checkout of its own (a temporary one is fine), never in an executor's or the monitor's checkout. Any
session may run it, the monitoring session included. It starts with the monitor's status, then works through every
landed plan in `REPORTS.md`, oldest first, then every `[blocked]` plan in queue order. "Show blocked plans" and "check
for blocked plans" are the same flow limited to blocked plans, still starting with the monitor's status.

The monitor's status is one line, taken from the `## Status` section of `farhelm-plans-monitor-log.md` beside the
checkouts: when it last checked, or when and why it stopped. An idle monitor checks in at least every 110 minutes, so a
`running` line whose last check is more than about two and a half hours old means it has probably died; say so. A
missing log means no monitor has run on this machine, and nothing is landing. Add any flags `queue status` raises.

By default, review the landed plans on one page using the `interactive-artifact` skill; load and follow it by name. Do
not publish the page as a snippet or anywhere else unless the maintainer asks.

The page holds every report in `REPORTS.md`, oldest first, cut down to what the maintainer can act on; they made this
concise form the default on 2026-10-23. Each report gets, in this order: its PRs and summary line; one or two sentences
on what was done, in product terms; then only the plausible leftover bugs and follow-ups that matter: real gaps, known
wrong behavior, risky untested paths with a concrete consequence, and decisions the report or its Landing section says
are still needed, each of those marked "Decision needed:". One short sentence per item, and a plan with nothing left
says so. Leave out checks that ran, changelog and release bookkeeping, documentation polish, stale comments, test
hygiene, screenshot refreshes, what the landing already fixed, flakes already recorded, caveats that are design choices
the maintainer already made, and notes that something was not tried on a real Mac or in the real desktop app, unless the
untried case is particular or risky. A leftover that a later commit verifiably resolved may say so beside it. Then the
full report, collapsed; then a picker with an "approve" choice and a free-text field. Use the plan's slug as its stable
picker id. The summary and the leftovers are the session's own reading of the report and its Landing section, never new
claims. The skill's Submit and wait mechanism collects the choices, comments and suggestions.

Write the body fragment as `page.html` in a private directory inside your scratch directory (`AGENTS.md`, "Agent scratch
space"). Follow the skill for styling, serving, sharing the URL and any tunnel instructions, waiting for submissions,
status, replies and shutdown. Use its chosen port and tokenized URL. If the skill is unavailable or its server fails,
stop and report why.

Act on each submission:

- an "approve" pick is an approval (`queue approve`, as the approve item in this section says);
- a comment with an instruction is that instruction, whether or not "approve" is selected: a follow-up, a TODO.md entry,
  a question to answer, or anything else it asks for;
- an unselected report without a comment means the maintainer did not get to it. Leave it for later. It is never a
  follow-up or a rejection.

Comments and suggestions anchored to a report count as comments on that report; a suggested edit alone is not an
approval. Reply on the page where clarification or an answer is needed. After recording decisions, remove handled
reports and update the same page before waiting again; unchanged picks must not repeat queue actions. Finish the page
using the skill's completion procedure when the review ends or no reports remain.

The one-at-a-time walkthrough in this section is used when the maintainer asks for it. Blocked plans are still presented
one at a time in conversation, because each needs an answer.

When walking through a landed plan one at a time, start with plain-language context (`AGENTS.md`, "Talking to the
user"): what the work was about, in product terms. Then walk the maintainer through the report rather than pasting it:
what landed, the things they should know, the open questions and possible follow-ups, anything the Landing section adds
(what it landed alongside, fixes made while landing), and a link to the full report on GitHub,
`https://github.com/<OWNER/NAME>/blob/main/plans/reports/<slug>.report.md`. Then ask for a decision:

- approve: `queue approve`, which removes the plan's `REPORTS.md` line, its plan file and its report. The work stays on
  main; only its tracking ends.
- follow-up: the plan needs another round on top of what landed (a fix, a missing piece, a revert). Write the
  maintainer's words verbatim (plus any agreed restatement) to a file and run `queue follow-up --decision <file>`, which
  puts the plan back at the top of `INDEX.md` as `[pending]` with the follow-up in its Decisions. Its report stays, for
  the next round's "since the last review" section. Plans that ran after it no longer wait on it
  (`plans/.agents/queue.md`, "States, and the script that moves them").
- something that deserves its own plan or a TODO.md entry: that is a separate request handled the usual way (planning,
  or a TODO edit as `AGENTS.md`, "TODO.md and FLAKES.md", describes), after which this plan can be approved.
- leave it for later, which changes nothing.

For each blocked plan, start with the same context, then present the Blocked question, and ask for a decision:

- answer: the maintainer's words verbatim (plus any agreed restatement) to `queue answer --decision <file>`, which moves
  the question and the answer into Decisions and puts the plan back to `[pending]`;
- abandon (the abandoning paragraph at the end of this section);
- leave it for later.

A plan the monitor blocked while landing may already have some of its PRs on main; its question says which. Answering
sends it back to an executor, which builds on what landed.

Record each decision as it is made, so executors can pick up answered and followed-up plans while the review goes on.
End by summarizing what was approved, sent back and answered.

Abandoning a plan: only a plan still in `INDEX.md` can be abandoned (pending, blocked, or complete; a claimed one is
released first). Close its open PRs, then run `queue abandon`. If other plans run after it, ask the maintainer whether
they block (`--dependents block`, which gives each one a Blocked question about it) or drop the dependency
(`--dependents drop-dep`). Ask what happens to the plan's source (the TODO entry goes back to unplanned, or the triage
outcome's Execution back to `pending`, or something else), and land that as its own PR per `jjstack`. Abandoning leaves
whatever already reached main in place, so for a plan with merged PRs, ask whether a revert is wanted. A landed plan
cannot be abandoned: its work is on main, and undoing it is a follow-up.

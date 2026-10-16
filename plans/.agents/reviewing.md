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

By default, the landed plans are reviewed on one page rather than one at a time in conversation. The maintainer chose
this on 2026-10-14. Publish one `scode-snippets` snippet holding every report in `REPORTS.md`, oldest first. Each report
gets, in this order: its PRs and summary line; a TLDR written by the session preparing the page, at most three sentences
saying what the plan was about, in product terms; then only what the maintainer should know (risks, caveats, release
impact, checks that could not be run, follow-ups), stated as concisely as possible; then the full report; then an
"approve" checkbox and a free-text comment field. The TLDR and the highlights are the session's own reading of the
report and its Landing section, never new claims. A button at the bottom copies every choice as one block of text that
the maintainer pastes back into the session. Give them the snippet's link, then act on the pasted block:

- a ticked box is an approval (`queue approve`, as the approve item in this section says);
- a comment with an instruction is that instruction, whether or not the box is ticked: a follow-up, a TODO.md entry, a
  question to answer, or anything else it asks for. Ask in conversation only where the comment is unclear;
- an unticked report without a comment means the maintainer did not get to it. Leave it for later. It is never a
  follow-up or a rejection.

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

# Planning

How plans are written and put on the queue: the todo planning flow, plan to implement TODO items, and scheduling triage
outcomes through the planning system. The planner reads it before the clean-main starting check of any of these flows
(`plans/AGENTS.md`, "Flows and reading lists").

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Choosing what to plan: "todo planning flow"

"Todo planning flow" lets the maintainer choose entries to plan by ticking boxes on a local page instead of naming them.
It covers the Near term bucket unless the request names another one ("todo planning flow for maybe later"). The page
lists every entry in that bucket that has no plan yet, each as a short label and a one-to-two sentence summary, and its
"Copy code" button copies a line like `todo-plan near-term: pin-rust-toolchain, upgrade-epochs` for the maintainer to
paste back. `scripts/todo-picker.py` renders and serves the page; its header covers the details.

- Apply the clean-main boundary, then work from the latest `main@origin`, so the page shows TODO.md as it is now.
- Run `python3 scripts/todo-picker.py skeleton --bucket "<bucket>"` and save its output in your scratch directory
  (`AGENTS.md`, "Agent scratch space"). It lists every entry without a plan, in file order, and reports the skipped ones
  on stderr.
- Fill in each entry's `label` (a few words, used for the entry's slug in the pasted code) and `tldr` (what it is and,
  when the entry says so, why or the first step), in product terms per `AGENTS.md`, "Talking to the user". Leave the
  keys and their order alone.
- Start `python3 scripts/todo-picker.py serve --summaries <file>` as a background command and give the maintainer
  `http://127.0.0.1:14003/`. The script refuses a summaries file that no longer matches the bucket's unplanned entries,
  which catches TODO.md moving since the skeleton: rerun the skeleton and fill it in again. If something else holds the
  port, say so and ask which port to use (`--port`); never stop a process you did not start.
- When the maintainer pastes a `todo-plan` line, stop the server, map each slug back to its entry through the summaries
  file, and run "plan to implement" (`plans/.agents/planning.md`, "Planning: "plan to implement <TODO items>"") on
  exactly those entries. The pasted line is that request, so do not ask for confirmation; anything the maintainer writes
  alongside it is what they said about the entries.

## Planning: "plan to implement <TODO items>"

That request, or anything like it, starts the planning flow. Each named TODO entry gets its own plan, including when the
request identifies entries in bulk ("all the items in the near term bucket"). When some entries are very closely
related, you may propose merging them into one plan, but planning each entry alone is the default and merging needs the
maintainer's agreement. A merged plan references every entry it covers, and each of those entries references the plan.

Apply the clean-main boundary first, then work from the latest `main@origin`. An entry that already references a plan is
skipped and reported, not planned again; a request to revise an existing plan updates that plan's file in place, which
is allowed only while the plan is `pending` or `blocked`. Never reuse a slug that has ever existed under `plans/`:
`queue check-slug <slug>` refuses one, because an old plan's working log may still sit beside the checkouts and a new
plan with that name would resume from it.

For each plan, run `scode-build-goal` with the entry's text (and whatever the maintainer said about it) as the goal,
with these overrides on top of the skill's own rules:

- No-workhorse mode.
- Ask the up-front questions on a local interactive page (`plans/.agents/planning.md`, "Asking the up-front questions on
  a local page"), never as a prose batch in conversation, a published snippet, or one question at a time through a
  question tool.
- When the entry changes the GUI (the web UI or the desktop app), default to mocking up the proposed change on that
  page, next to the questions it settles, rather than describing a layout in words. Settle the design on the mockup,
  then write what was agreed into the goal file in words: the executor cannot ask about it later, and the page lives
  outside the repository. Skip the mockup when the maintainer says to, or when the change has no visible design to
  decide.
- In the review gate menu, add a fresh-context Opus 5.5 agent at high effort with the skill's general review charter, as
  a pinned-model option, and present it first, as the default. The goal file records the choice as a demand for that
  model and effort, like any pinned option.
- The goal file is `plans/queue/<slug>.md` in this checkout, not the parent-directory default.
- The goal file contains no absolute paths: the repository may become public, and the plan will likely be executed from
  a different checkout than the one that wrote it. Refer to the repository as "the checkout the executing session runs
  in" and to everything in it by repo-relative path. Paths outside the repository are stated as rules
  (`plans/.agents/outside-files.md`, "Files outside the repository"), and the executing session resolves them before it
  starts.
- The log is `farhelm-plan-<slug>-log.md` in the parent directory of the executing checkout, stated in the goal file as
  that rule. Its existence keeps the goal self-resumeable the way the skill intends.
- The delegation paragraph names the sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", lists as exempt from
  the no-delegation demand.
- Stack and PRs: the stack's base is `main@origin`, or the plan's own open PRs when it resumes, never another plan's
  PRs; set up per `plans/.agents/executing.md`, "Executing one plan". Bookmarks are `plan/<slug>/<nn>-<short-name>`. PRs
  stay drafts; the executor never marks one ready and never merges. Within a run, a PR that needs correcting is
  restructured rather than corrected on top, and that applies to all of the plan's own open PRs, including ones an
  earlier run built.
- The plan never edits `plans/`: its state moves only through the script, as `plans/.agents/executing.md`, "Executing
  one plan", describes.
- A TODO plan's last code PR removes the TODO entries it covers.
- The done criterion: every PR exists and has passed its review gate, and the latest `## Decisions` entry, if any, is
  satisfied. Delivering the report is the executor's closing step, not part of the goal.
- The unattended fallback blocks per `plans/.agents/executing.md`, "Executing one plan" (Blocking), and states the rule
  in `plans/AGENTS.md`, "Unattended means no questions", in the goal file's own words: the executor never asks the
  maintainer anything in conversation, not even an optional question or offer; a fork is decided and logged or it is a
  block; things to know and possible follow-ups go into the report.
- If the plan depends on another plan's work being on main first, the goal file says which, and so does its `INDEX.md`
  line.

New plans go at the end of `INDEX.md` unless the maintainer places them elsewhere.

NOTE: Queue order is not execution order. Several executors drain the queue at once, so any eligible plan may run at the
same time as any other, and a plan listed later can start, deliver and land before one listed earlier
(`plans/.agents/executing.md`, "Picking: "pick a plan to execute", "execute the next plan"", prefers the oldest eligible
plan, but skips it when it would conflict with unlanded work). Listing one plan below another orders nothing. The only
ordering between plans is `(after x.md)`, and that holds the dependent plan back until the monitor has landed `x.md`,
which waits on the maintainer only if `x.md` or its landing blocks. When pieces of work must be built in a particular
order, or one of them touches much of what the others touch (a broad rename that every other change would rebase across,
say), keep them in one plan whose goal builds them as one stack in that order; for separate TODO entries that is a merge
to propose, as the first paragraph of this section says. Do not split such work into separate plans on the assumption
that a later plan runs after an earlier one: without a dependency they may run concurrently and conflict, and with one
the later work idles until the earlier work lands.

When every plan in the request is written, land them as one commit and one PR per the `jjstack` skill, on top of the
latest `main@origin`: the plan files, their `INDEX.md` lines, and the TODO.md references, all together. Validate with
`dprint check` on the changed files, `queue check`, and `queue check --base main` after the final rebase. A `--base`
violation on a line or report the PR did not touch means main moved under the check (another executor claimed or
delivered): fetch, rebase, and run it again. Ask the maintainer to approve, and merge per `jjstack` once they do. If the
merge is refused because main moved, rebase, check again, and retry. A plan that only lives in a working copy does not
exist as far as an executor is concerned, because executors only read plans from main.

## Asking the up-front questions on a local page

Every planning flow in this file asks its up-front questions this way. The maintainer chose it on 2026-10-15, because
answers given in the terminal cannot be revisited or edited. One page holds every open question for every plan in the
request, served from this machine on loopback only, the way the todo picker is. Here's what it needs:

- One bordered box per plan, headed by the plan's name in product terms and its slug, with the heading kept in view
  while scrolling. A bare id prefix on each question does not tell the maintainer which plan it is about.
- Each question offers its picks, the recommended one first and marked as such but not preselected, plus a free-text
  comment.
- Beside every question, a "Copy code" button that copies the answers for the whole page as one block for the maintainer
  to paste back. A question with neither a pick nor a comment is left out of the block and stays open. Show the block on
  the page as well, since the clipboard can be refused.
- Facts that are not questions (the delegation mode, where the plan files go) are stated on the page or in chat, not
  asked.

Write the page in your scratch directory (`AGENTS.md`, "Agent scratch space") and serve it with any static file server
bound to `127.0.0.1` on port 14003, the todo picker's port, after stopping the picker; the maintainer keeps that one URL
open in a browser. For example, `python3 -m http.server 14003 --bind 127.0.0.1 -d <dir>` as a background command. If
something else holds the port, say so and ask which port to use; never stop a process you did not start. Put the page's
URL in the last message of the turn, after every tool call.

After acting on a pasted block (research, follow-up questions), update the same page in place: the questions still open,
any new follow-ups, and the settled answers shown read-only, so the maintainer can see what is still needed.

## Planning triage outcomes: "use the planning system to schedule these"

Executing triage outcomes normally does not involve `plans/` at all: "execute triage outcomes", or turning them into a
goal with `scode-build-goal` directly, follows `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage
outcomes" and the skill as usual. This section applies only when the maintainer explicitly asks for the planning system,
for example "use the planning system to schedule these for execution".

Such a request makes one plan per outcome ("these" after a triage session means the outcomes decided in it; ask if it is
unclear which). Grouping outcomes that are extremely closely related into one plan is something to propose, and needs
the maintainer's agreement, as in the TODO flow. Each plan's goal is to execute its outcome per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes" (one PR per outcome) and names the
outcome by its `TRIAGE_OUTCOMES.md` heading. Only outcomes whose Execution is `pending` can be scheduled; skip and
report the rest.

Plan them as in the TODO flow (`plans/.agents/planning.md`, "Planning: "plan to implement <TODO items>""), with the same
`scode-build-goal` overrides, except:

- The decisions are already recorded in `TRIAGE_OUTCOMES.md`, so the up-front questions cover only what the ledger
  leaves open for unattended execution, not the outcomes themselves.
- There are no TODO entries to remove; the outcome's own PR records its execution in the ledger, as
  `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes", requires.
- The planning PR carries the plan files, their `INDEX.md` lines, and the `` planned in `plans/queue/<slug>.md` ``
  Execution update for each scheduled outcome, in place of TODO references.

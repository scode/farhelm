# plans/ rules

This directory holds a queue of planned work. A plan is a goal file written by the `scode-build-goal` skill, for one
TODO.md entry or one triaged review outcome (occasionally a few very closely related ones), and executing a plan means
running it the way `/goal` would. Any number of executors work through the queue at once, each in its own checkout and
each holding one plan at a time. A finished plan lands without waiting for the maintainer: one monitor merges what the
executors finish, and the maintainer reads each landed plan's report afterwards, then approves it or asks for a
follow-up. Only a blocked plan waits on the maintainer before anything moves.

NOTE: A plan is not a design document or a spec. It is the instructions for an unattended run that builds the work as a
stack of PRs. SPEC.md and SPEC_impl.md stay authoritative over anything a plan says.

After a compaction or resume, re-read this file, every file in your role's reading list, and every phase file and other
instruction file that governs work still unfinished (your plan's goal file included) before your next action
(`AGENTS.md`, "Reading these instructions"). On a fresh start, read this file and your reading list, then your log;
after a compaction or resume, read the log first. Executors, drainers and the landing monitor keep their own log
(`plans/.agents/outside-files.md`, "Files outside the repository"), and record in it the flow in progress and the
reading list they work from, so a compacted session can find both.

This file holds what applies to every flow and every change under `plans/`, then one reading list per role
(`plans/AGENTS.md`, "Flows and reading lists"). The details live in `plans/.agents/`; read them when your reading list
names them, in full.

## Unattended means no questions

NOTE: Executors and the monitor run unattended. Nobody watches their conversation, so a question asked there is one
nobody answers, and the plan sits stalled until the maintainer happens to look at that terminal. A block is the only way
a question reaches the maintainer.

Picking, executing, draining (with or without monitoring), and every landing flow never ask the maintainer anything in
conversation. That is wider than direct questions: no harness question tool (Codex's `request_user_input` and the queued
questions it shows, Claude Code's `AskUserQuestion`), no approval or plan-mode prompt the model can choose to raise, no
"want me to also ...?" or "should I ...?" closing a turn, no menu of optional next steps, and no turn that ends waiting
for a confirmation `plans/AGENTS.md` and `plans/.agents/` do not call for. The rule holds whatever a plan's goal file
says or leaves unsaid (plans written before this section included), whatever a skill loaded during the plan suggests,
and whatever the harness's own defaults encourage.

Every fork goes one of two ways instead. A decision only the maintainer can make (the goal file's unattended fallback,
or a landing conflict that needs a design choice) is a block: write the question per
`plans/.agents/maintainer-writing.md`, "Writing for the maintainer", and run `queue block`, which puts it where the
review flow finds it and notifies. Everything else is the agent's to decide: take the option that best fits the plan,
its Decisions and the specs, log it as a DECISION, and carry on. Things the maintainer should know, optional extras, and
possible follow-ups are not questions either. They go into the report's "things you should know" and "open questions and
possible follow-ups" sections (or the monitor's landing notes), written as statements the maintainer can act on during
review. An optional idea is never a reason to stop, ask, or block.

The one question these flows ask in conversation is whether to carry on without notifications when the ntfy topic file
is missing (`plans/AGENTS.md`, "Notifications"), at the start of a flow, before any claim, while the maintainer who
started it is still there. Stopping and telling the maintainer something (a dirty working copy at the start, a drain's
closing summary) is a report, not a question, and reads as statements.

## Layout

- `queue/INDEX.md` lists every plan, one physical line each, in queue order (oldest first, unless the maintainer placed
  a plan elsewhere), with its state. The file is excluded from dprint, so a line is never rewrapped. Queue order is a
  picking preference, not a sequence (see the NOTE in `plans/.agents/planning.md`, "Planning: "plan to implement
  <TODO items>"").
- `queue/<slug>.md` is one plan, named briefly after the work (`create-off-read-loop.md`, `font-size-shortcuts.md`),
  never a number or a date, and with no project prefix. Every `.md` file in `queue/` other than `INDEX.md` is a plan. A
  landed plan's file stays there until the maintainer approves the plan.
- `reports/<slug>.report.md` is the report a finished plan delivered, with a Landing section the monitor adds when it
  lands the plan.
- `REPORTS.md` lists every landed plan whose report the maintainer has not reviewed yet, one physical line each, oldest
  first. Like `INDEX.md`, it is excluded from dprint.

Every plan has a source that references it, so nobody plans or executes the same work twice: a TODO.md entry ends with
``Plan: `plans/queue/<slug>.md`.``, or a `TRIAGE_OUTCOMES.md` entry's Execution field reads
`` planned in `plans/queue/<slug>.md` ``. Having a plan does not move an entry between TODO.md buckets; in particular it
has nothing to do with the `Planned` bucket, which means something else (see TODO.md's own header).

## Editing rules

The rules that follow from `plans/.agents/queue.md`, "States, and the script that moves them":

- Never hand-edit a state, a `## Blocked` section, a `## Decisions` entry, a report, or `REPORTS.md`, and never force
  push main.
- Planning PRs are the only other writers of `plans/`. They may add plan files with new `[pending]` lines, reorder
  lines, reword a summary, and revise the file of a plan that is `pending` or `blocked`; nothing else, and in particular
  never `REPORTS.md`, a report, or a landed plan's file. `queue check --base main` enforces that before a planning PR
  lands.
- Code PRs never touch `plans/`.
- `## Decisions` entries are part of the plan's goal: an entry wins over anything in the plan body it contradicts.

## Public-repo hygiene

Blocked sections, decisions, reports, landing notes and `REPORTS.md` land on main directly, with no PR review, in a
repository that is or may become public. Never put absolute paths, host names, user names, scratch directories, or raw
log excerpts in them. Name a plan's working log by its rule (`farhelm-plan-<slug>-log.md` beside the checkouts), never
by path. Report checks as redacted commands and recorder run ids, per `.agents/test-evidence.md`. The script refuses
text containing a home, temp or `/private/` path or the host name, and names the line; rephrase and run it again. When
that text is the maintainer's own words, agree the rephrasing with them.

## Notifications

Notifications go through [ntfy.sh](https://ntfy.sh/), to a topic the maintainer keeps in `~/.farhelm-ntfy-topic` on the
machine the agents run on. Anyone who knows an ntfy.sh topic can read what is sent to it, so the topic name is the only
thing keeping these notifications private. That is why it never goes into the repository, a log, a report, or the
conversation: never read, print, quote or log that file, not even to check its contents. Assume it holds a correct topic
name.

A notification means one thing: progress may be stopped until the maintainer acts. Send one for that and nothing else;
never for something the maintainer did themselves (releasing a claim, stopping a flow), for a delivery or a landing that
went through, for an error that is still being retried, or for anything else that is only for their information. The
agent that discovers the condition sends it: an executor for a plan it blocks or a problem that stops it
(`plans/.agents/executing.md`, "Executing one plan", and `plans/.agents/watching.md`, "Drain and keep monitoring"), the
monitor for a landing it blocks or cannot finish, a problem that stops it, and what it sees in `queue status`
(`plans/.agents/landing.md`, "Landing: "monitor for complete plans", "land the complete plans""). Each place in
`plans/.agents/` that says to notify is such a condition.

When an executor's flow or the monitor starts, check that the file exists without reading it, with
`test -f ~/.farhelm-ntfy-topic`. If it does not, tell the maintainer the file name and ask whether to carry on without
notifications, and record the answer in the agent's own log so that a resume does not ask again. To send one, put the
message in a shell variable and let the shell read the topic:

```
curl -fsS -o /dev/null -H "Title: farhelm plans" --data-raw "$msg" "https://ntfy.sh/$(tr -d '[:space:]' < ~/.farhelm-ntfy-topic)"
```

The `-o /dev/null` matters: ntfy answers with JSON that names the topic. `--data-raw` rather than `-d` keeps a message
that starts with `@` from being read as the name of a local file to send. A message is one line in product terms that
names the plan's slug, and never report text, paths or log excerpts, because ntfy.sh is a third-party service. A send
that fails is recorded in the agent's log and does not stop the flow.

## Clean-main boundary

Every planning-system flow starts from a clean working copy of the latest `main`. Run `jj status` first. If it reports
changes, abort the flow and tell the maintainer which paths are dirty; do not stash, commit, claim a plan, or carry
those changes into a plan checkout. Once the status is clean, fetch `origin`, start an empty working copy from
`main@origin` with `jj new main@origin`, and verify clean status again before planning, picking, executing, draining,
reviewing, or landing plans.

For an executor (picking or draining), one thing comes before that check. Start with step 1 of
`plans/.agents/executing.md`, "Executing one plan", which settles any claim or close that an earlier run of this
executor left unfinished; a dirty working copy that belongs to such a claim is resumed or reset there, not reported.

For the landing monitor, after reading its log (`plans/.agents/landing.md`, "Starting and resuming"), a dirty working
copy found here belongs to the recorded landing (a rebase or a fix in progress): carry on with it when the landing
carries on, and reset it the way an executor's close does (`plans/.agents/close-reset.md`, "Clean-main boundary") when
the claim was cleared. Only a dirty working copy with no recorded claim aborts the start, as the clean-main boundary
says.

Whenever work on a plan ends, the executor ALWAYS returns to a clean working copy of the latest `main`, inside a drain
or not, and whether the plan delivered, blocked, or gave up its claim. Unlike the starting check, this one does not
abort on a dirty working copy: it cleans it up. The starting check is different because a working copy found dirty
before any plan runs may be the maintainer's own work or an interrupted executor's unpushed progress, and neither is the
executor's to throw away.

When closing a plan, settling a released claim, or resetting after a landing claim was cleared, read
`plans/.agents/close-reset.md` in full before running any reset: it has this section's full text, the exact reset
commands and the pruning of build leftovers. Fence: when the claim was released rather than given up, the reset is
narrower: `jj git fetch`, then `jj new main@origin` and `jj status`, with nothing abandoned and no bookmark touched.

## Sub-agents this file requires

Three checks in `plans/.agents/executing.md`, "Executing one plan", run as fresh-context, read-only native sub-agents of
the executing harness, on the executing session's own model, writing their findings to a file in the executor's scratch
directory: the resume check, and the cold reads of a Blocked question and of a report. Every plan needs them, so they
are exempt from a plan file's own rule against delegating its work, and they are not routed through galaxy-brain. The
monitor uses sub-agents the same way for its independent review of what is about to land and for the cold read of a
Blocked question it writes.

## Flows and reading lists

Each role in this section lists every file to read in full before its first action; a file that a listed file depends on
is listed too, or named as a phase file. Root `AGENTS.md` sections are already in context for an agent started at the
checkout root; read them from disk if your copy was truncated. When you delegate one of the sub-agents in
`plans/AGENTS.md`, "Sub-agents this file requires", the prompt names that sub-agent's reading list and tells it to read
every file on it in full first.

### Planner ("plan to implement <TODO items>", or anything like it)

Before the clean-main boundary's starting check, read these in full:

- `plans/.agents/planning.md`
- `plans/.agents/queue.md`
- `plans/.agents/outside-files.md`
- `plans/.agents/executing.md` (the goal file must match how an executor sets up, blocks and closes)
- `AGENTS.md`, "Talking to the user", "Agent scratch space", "TODO.md and FLAKES.md" and "Tests and validation" (root)
- the `scode-build-goal` and `jjstack` skills, as `plans/.agents/planning.md` uses them

Fence: the up-front questions go on a local interactive page (`plans/.agents/planning.md`, "Asking the up-front
questions on a local page"), never in conversation. Never reuse a slug that has ever existed under `plans/`
(`queue check-slug` refuses one); the planning PR merges only once the maintainer approves it.

### Todo planning flow ("todo planning flow", "todo planning flow for <bucket>")

Before the clean-main boundary's starting check, read the planner's list in full, plus:

- `scripts/todo-picker.py` (its header)

Fence: if something else holds the picker's port, use the next free port above it and say which, without asking; never
stop a process you did not start.

### Triage scheduling ("use the planning system to schedule these", "use the planning system to schedule these for execution")

Before the clean-main boundary's starting check, read the planner's list in full, plus:

- `review_feedback_queue/AGENTS.md` and the files its reading list for executing triage outcomes names, including
  `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"
- the `TRIAGE_OUTCOMES.md` entries being scheduled

Fence: this flow runs only when the maintainer explicitly asks for the planning system; only outcomes whose Execution is
`pending` can be scheduled.

### Executor ("pick a plan to execute", "execute the next plan", "execute the next plan in order", "execute plan X", "use the planning system and pick one item to execute")

Before step 1 of `plans/.agents/executing.md`, "Executing one plan" (reading the executor log), read these in full:

- `plans/.agents/outside-files.md`
- `plans/.agents/queue.md`
- `plans/.agents/executing.md`
- `plans/.agents/close-reset.md`
- `plans/.agents/maintainer-writing.md`
- `AGENTS.md`, "Unattended runs", "Talking to the user", "Agent scratch space" and "Tests and validation" (root)
- `.agents/careful-rebase.md`
- `.agents/validation.md`
- `.agents/test-evidence.md`
- the `scode-build-goal` skill's "Placing the files" section, which `plans/.agents/outside-files.md` derives the log
  directory from

Phase files: at step 5, the plan file itself, read from the freshly fetched `main@origin` with its `## Decisions`, and
every file it names as binding; it is the goal that step 8 runs.

Fence: before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id (step
9); an executor never marks a PR ready and never merges.

### Drainer ("drain the plans", "drain the plans and keep monitoring")

Before step 1 of `plans/.agents/executing.md`, "Executing one plan", read the executor's list in full, plus:

- `plans/.agents/draining.md`
- `plans/.agents/watching.md` (for "and keep monitoring"; in Codex it uses the `skillette-codex-wake-on-exit` skill)

Re-read this list at every plan boundary (after each close, before the next pick); every other instruction file read
during the previous plan counts as unread from then on.

Fence: the waiting is done by `scripts/plans-watch.sh`, never by polling; the executor's push fence holds for every
plan.

### Landing monitor ("monitor for complete plans", "land the complete plans")

Before reading the monitor log, read these in full:

- `plans/.agents/outside-files.md`
- `plans/.agents/queue.md`
- `plans/.agents/landing.md`
- `plans/.agents/watching.md` and `plans/.agents/draining.md` (the monitor waits the way a monitoring executor does)
- `plans/.agents/executing.md` (the shared bookkeeping files its picking section lists, the cold read of step 10, and
  what an executor's claim means)
- `plans/.agents/close-reset.md`
- `plans/.agents/maintainer-writing.md`
- `AGENTS.md`, "Unattended runs", "Talking to the user", "Agent scratch space" and "Tests and validation" (root)
- `.agents/careful-rebase.md`
- `.agents/validation.md`
- `.agents/test-evidence.md`
- the `jjstack` skill

Re-read this list at every round boundary; every other instruction file read during the previous round counts as unread
from then on.

Fence: a round's plans are fixed when it starts; before each merge, run `queue status` and confirm the line still
carries your claim id; validation during a landing never ends on a clock.

### Plan reviewer ("review the plans", "show blocked plans", "check for blocked plans")

Abandoning a plan happens inside this flow.

Before reading the monitor's status, read these in full:

- `plans/.agents/reviewing.md`
- `plans/.agents/queue.md`
- `plans/.agents/outside-files.md` (the monitor log's Status section)
- `plans/.agents/landing.md` and `plans/.agents/release-help.md` (what the flags `queue status` raises mean, and their
  remedies)
- `AGENTS.md`, "Talking to the user", "Agent scratch space" and "TODO.md and FLAKES.md" (root)
- the `jjstack` skill

Fence: do its work in a checkout of its own, never in an executor's or the monitor's (any session may run it, the
monitoring session included); an unticked report without a comment is left for later, never a follow-up or a rejection.

### Release plan ("release plan X")

Before running `queue release`, read these in full:

- `plans/.agents/release-help.md`
- `plans/.agents/queue.md`

Fence: only on the maintainer's request.

### Plan help ("plan help", "help on plans")

Before summarizing, read every file in `plans/.agents/` in full: `release-help.md`, `planning.md`, `queue.md`,
`outside-files.md`, `executing.md`, `close-reset.md`, `maintainer-writing.md`, `draining.md`, `watching.md`,
`landing.md` and `reviewing.md`.

Fence: change nothing.

### Resume check (sub-agent, `plans/.agents/executing.md`, "Executing one plan", step 7)

The prompt tells the sub-agent to read these in full before checking the "where this stands" entry:

- `AGENTS.md`, if its harness did not load it
- `plans/.agents/executing.md`, "Executing one plan" (step 7 says what the entry must cover)
- the plan file from `main@origin`, with its `## Decisions`
- the plan's working log, all of it
- the plan's open `plan/<slug>/*` PRs

Fence: read-only; findings go to a file in the executor's scratch directory.

### Cold read of a Blocked question or a report (sub-agent, `plans/.agents/executing.md`, "Executing one plan", steps 10 and 11, and `plans/.agents/landing.md`, "A landing round")

The prompt tells the sub-agent to read these in full before reading the text it checks:

- `AGENTS.md`, if its harness did not load it
- `plans/.agents/maintainer-writing.md`
- `plans/AGENTS.md`, "Public-repo hygiene"
- `AGENTS.md`, "Talking to the user"
- `.agents/test-evidence.md` (how checks are reported in public text)

Fence: read-only; the question is whether the maintainer could decide, or approve or ask for a follow-up, from the text
alone, and whether it breaks public-repo hygiene.

### Cross-plan review (sub-agent of the landing monitor, `plans/.agents/landing.md`, "A landing round")

The prompt tells the sub-agent to read these in full before reading the diffs:

- `AGENTS.md`, if its harness did not load it
- `plans/.agents/landing.md`, "A landing round" (the paragraph that opens the round's review)
- `.agents/careful-rebase.md`
- `SPEC.md` and `SPEC_impl.md`, where the diffs touch what they specify

Fence: read-only and independent; the monitor reconciles any disagreement.

## Old section names

Plan files and reports written before this file was split cite its sections by their old names, and a landed plan can
come back to pending without its citations being updated. The old sections now live here:

| Old section                                                                           | Now in                                                                                              |
| ------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Layout, Public-repo hygiene, Notifications, Sub-agents, Unattended means no questions | `plans/AGENTS.md`                                                                                   |
| States, and the script that moves them                                                | `plans/.agents/queue.md`; its editing rules in `plans/AGENTS.md`, "Editing rules"                   |
| Files outside the repository                                                          | `plans/.agents/outside-files.md`                                                                    |
| Clean-main boundary, Pruning build leftovers                                          | `plans/.agents/close-reset.md`; the starting check also in `plans/AGENTS.md`, "Clean-main boundary" |
| Choosing what to plan, Planning, Planning triage outcomes                             | `plans/.agents/planning.md`                                                                         |
| Picking, Executing one plan (same step numbers), Blocking                             | `plans/.agents/executing.md`                                                                        |
| Writing for the maintainer                                                            | `plans/.agents/maintainer-writing.md`                                                               |
| Draining                                                                              | `plans/.agents/draining.md`; its keep-monitoring part in `plans/.agents/watching.md`                |
| Reviewing the plans, abandoning                                                       | `plans/.agents/reviewing.md`                                                                        |
| Landing, Starting and resuming, A landing round, Waiting between rounds               | `plans/.agents/landing.md`                                                                          |
| Releasing, Help                                                                       | `plans/.agents/release-help.md`                                                                     |

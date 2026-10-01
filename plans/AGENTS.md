# plans/ rules

This directory holds one plan per unit of planned work, plus an index. A plan is a goal file written by the
`scode-build-goal` skill, for one TODO.md entry or for a batch of triaged review feedback outcomes, so executing a plan
means running it the way `/goal` would. `INDEX.md` is the authoritative order in which plans are executed.

NOTE: A plan is not a design document or a spec. It is the instructions for an unattended run that builds the work as a
linear stack of PRs. SPEC.md and SPEC_impl.md stay authoritative over anything a plan says.

## Files

- `INDEX.md` lists every plan file, one line each, in execution order: ``- [pending] `<slug>.md` — <one-line summary>``,
  with ``(after `<other>.md`)`` appended when the plan depends on another plan being built first. The status is
  `[pending]` until the plan's final PR changes it to `[executed]`. It must always match the plan files in this
  directory. Updating it is part of every change here, not a follow-up.
- `<slug>.md` is one plan, named briefly after the work (`create-off-read-loop.md`, `font-size-shortcuts.md`), never a
  number or a date. The directory is dedicated to this project, so the slug has no project prefix. Every `.md` file here
  other than `AGENTS.md`, `CLAUDE.md`, and `INDEX.md` is a plan.

Every pending plan has a source that references it, so nobody plans or executes the same work twice:

- A TODO plan corresponds to a TODO.md entry, and that entry references its plan with a trailing
  ``Plan: `plans/<slug>.md`.`` sentence.
- A triage plan corresponds to a batch of decided entries in root `TRIAGE_OUTCOMES.md`, and each of those entries'
  Execution field reads `` planned in `plans/<slug>.md` `` until its own PR records the execution.

Having a plan does not move an entry between TODO.md buckets; in particular it has nothing to do with the `Planned`
bucket, which means something else (see TODO.md's own header).

The plan's final PR marks its `INDEX.md` line `[executed]`. For a TODO plan, the same PR removes the TODO entry; for a
triage plan, the final outcome's own PR carries the marker, since triage execution allows no separate bookkeeping PR. It
does not delete the plan file: the agent executing the plan still reads that file as its goal while the final PR is
reviewed and fixed, and after a crash or compaction. An `[executed]` line on main therefore means the plan's work has
landed. Executed plans stay until the user asks to delete them (see Cleanup).

### Files outside the repository

Execution keeps its state in the parent directory of the checkout, next to it and out of the repository, the way
`scode-build-goal` places goal logs (derive the parent the way its Placing the files section does). If that derivation
does not yield a usable directory, stop and tell the user rather than picking a temporary location. The names carry a
fixed `farhelm-` prefix rather than the checkout's name, so a session in a sibling checkout can still find and read
them:

- `farhelm-plans-log.md`: the executor's own state, described under Executing.
- `farhelm-plan-<slug>-log.md`: the plan's working log, per the goal file's resume protocol.
- `farhelm-plan-<slug>-report.md`: a copy of the plan's final report, described under Reports.

## Planning: "plan to implement <TODO items>"

That request, or anything like it, starts the planning flow. Each named TODO entry gets its own plan, including when the
request identifies entries in bulk ("all the items in the near term bucket"). When some entries are very closely
related, you may propose merging them into one plan, but planning each entry alone is the default and merging needs the
user's agreement. A merged plan references every entry it covers, and each of those entries references the plan.

Fetch first, and work from the latest `main@origin`. An entry that already references a plan is skipped and reported,
not planned again; a request to revise an existing plan updates that plan's file in place. Never reuse a slug that has
ever existed under `plans/` (check `git log origin/main -- plans/<slug>.md`), because the executor's files for the old
plan may still sit beside the checkout and a new plan with the same name would resume from them.

For each plan, run `scode-build-goal` with the entry's text (and whatever the user said about it) as the goal, with
these overrides on top of the skill's own rules:

- No-workhorse mode.
- In the review gate menu, add a fresh-context Opus 5.5 agent at high effort with the skill's general review charter, as
  a pinned-model option, and present it first, as the default. The goal file records the choice as a demand for that
  model and effort, like any pinned option.
- The goal file is `plans/<slug>.md` in this checkout, not the parent-directory default.
- The goal file contains no absolute paths. The repository may become public, and the plan will likely be executed from
  a different checkout than the one that wrote it. Refer to the repository as "the checkout the executing session runs
  in" and to everything in it by repo-relative path. Paths outside the repository are stated as rules (Files outside the
  repository above), and the executing session resolves them before it starts.
- The log is `farhelm-plan-<slug>-log.md` in the parent directory of the executing checkout, stated in the goal file as
  that rule. Its existence keeps the goal self-resumeable the way the skill intends.
- The stack's base is not main but the tip of the plan stack, per Executing below; the goal file says so and points
  here.
- The skill's rule against stacking a correction on top instead of restructuring applies only to the PRs this run of the
  plan builds. PRs already in the plan stack, from earlier plans or from an earlier run of this plan that blocked, are
  the base and are not rewritten.
- The done criterion adds that the plan's final PR marks its `INDEX.md` line `[executed]` and removes the TODO entries
  it covers (for a triage plan, see Planning triage outcomes below). The plan file itself stays.
- If the plan depends on another plan being built first, the goal file says which, and so does its `INDEX.md` line.

New plans go at the end of `INDEX.md` unless the user places them elsewhere.

When every plan in the request is written, land them as one commit and one PR per the `jjstack` skill, on top of the
latest `main@origin` and never on top of a plan stack: the plan files, their `INDEX.md` lines, and the TODO.md
references, all together. Validate with `dprint check` on the changed files. Ask the user to approve, and merge per
`jjstack` once they do. A plan that only lives in a working copy does not exist as far as an executor is concerned,
because executors only read plans from main.

## Planning triage outcomes: "use the planning system to schedule these"

Executing triage outcomes normally does not involve `plans/` at all: "execute triage outcomes", or turning them into a
goal with `scode-build-goal` directly, follows root AGENTS.md (Execute triage outcomes) and the skill as usual. This
section applies only when the user explicitly asks for the planning system, for example "use the planning system to
schedule these for execution".

Such a request makes one plan for the whole batch the user names ("these" after a triage session means the outcomes
decided in it; ask if it is unclear which). The plan's goal is to execute exactly those outcomes per root AGENTS.md's
Execute triage outcomes, which still means one PR per outcome in a single stack; the goal names each outcome by its
`TRIAGE_OUTCOMES.md` heading. Only outcomes whose Execution is `pending` can be scheduled; skip and report the rest.

Plan it as in the TODO flow above, with the same `scode-build-goal` overrides, except:

- The decisions are already recorded in `TRIAGE_OUTCOMES.md`, so the up-front questions cover only what the ledger
  leaves open for unattended execution, not the outcomes themselves.
- The done criterion: every scheduled outcome's PR exists per Execute triage outcomes, and the last of those PRs also
  marks the plan's `INDEX.md` line `[executed]`. There are no TODO entries to remove.
- The planning PR carries the plan file, its `INDEX.md` line, and the `` planned in `plans/<slug>.md` `` Execution
  update for each scheduled outcome, in place of TODO references.

## Executing: "execute the next plan"

Only one executor runs at a time; there is no claim mechanism, and starting a second executor while one is active is a
mistake. Executed plans form one linear plan stack: each plan's PRs stack on top of whatever PRs are already in it. The
executor does not merge that stack; landing it is the user's job, bottom first, whenever they choose, usually from
another session.

The plans log (`farhelm-plans-log.md`) is how the executor knows where it is, and the executor is its only writer. It
records:

- the checkout the executor runs in. Unpushed work exists only there, so an executor started in a different checkout
  while the log names another one stops and asks the user instead of continuing;
- the flow in progress (one plan, a drain, or a drain that keeps monitoring) and, while sleeping, the next wake time;
- the plan stack as a chain of PRs, bottom first, each with its bookmark, PR number, and the plan it belongs to. One
  plan can own PRs in several places in the chain, because a plan that blocks and later resumes continues at the tip;
- every plan the executor has started whose `INDEX.md` line on main is not yet `[executed]`, with its state: active,
  complete (all its PRs built and through their review gates), or blocked;
- for a blocked plan, the question it currently waits on, the git blob hash of its plan file as it was when it blocked,
  and the user's answer to that question once one arrives.

Read it at the start of every round and after any compaction or resume, before anything else. Each plan's own progress
lives in that plan's log, not here.

Executing the next plan means:

1. Fetch, then reconcile the plans log with GitHub, which is authoritative about which plan PRs remain. Drop merged PRs
   from the chain. Retire a plan from the log only once its `INDEX.md` line on `main@origin` reads `[executed]`, which
   means its final PR has landed; an active or blocked plan stays whatever else of it has landed. When bookmarks moved
   on the remote (a landing, or an edit from another session), take the remote's positions per `jjstack` rather than
   pushing the local ones over them, and rebase local work above them onto the moved commits. A plan PR closed without
   merging is the user rejecting it: mark that plan blocked with that as its question and its plan file's blob hash,
   notify, and end the flow, because later PRs sit on top of the rejected one and what happens to them is the user's
   call.
2. If the plans log records an active plan, resume it: it was interrupted, and the steps below continue it rather than
   selecting another.
3. Otherwise select: the first `[pending]` line of `INDEX.md` as it reads on `main@origin` whose plan the plans log does
   not record as complete, and is not blocked. A blocked plan is eligible again once an answer to its current question
   is recorded, or once its plan file's blob on `main@origin` differs from the hash recorded when it blocked. A plan
   whose `INDEX.md` line names a dependency that is blocked or not yet built is skipped too. A dependency is built once
   it is `[executed]` on main, recorded as complete, or no longer listed in `INDEX.md` at all (Cleanup only removes
   executed plans, and slugs are never reused). If nothing is eligible, the round is over.
4. Set up the base. With an unmerged plan stack, rebase it onto `main@origin` if main moved, as a careful rebase (root
   AGENTS.md), and work on top of its tip. With none, start a new change on `main@origin`. Conflicts in `INDEX.md` or
   `TODO.md` from plans landing concurrently are mechanical: keep both sides. If the rebase hits a conflict that needs a
   design decision, stop: record the question in the plans log, notify, and end the flow, since every plan shares that
   base and none can proceed.
5. Record the plan as active in the plans log before doing any work on it.
6. Run `plans/<slug>.md`, as it reads in the working copy on top of that base, as the goal. For a single "execute the
   next plan", set the harness's goal to it if the harness lets the model set its own goal; otherwise, and always within
   a drain, behave exactly as if it were the harness-provided goal under the outer request. A drain does not register
   each plan as a harness goal, because a blocked plan could not be cleared to make room for the next one.
7. The plan is complete only when its goal's done criterion holds: every PR exists and has passed its review gate. When
   it blocks instead (per the goal file's unattended fallback, the remaining work waits on a decision only the user can
   make), record the question and the plan file's blob hash in the plans log, replacing any earlier question and
   clearing any earlier answer, and notify. A blocked plan's finished PRs stay in the plan stack; its `INDEX.md` line
   stays `[pending]`, since its final PR was never built.
8. Close the plan either way: write its report (see Reports), give the final report in chat as usual, write a closing
   entry in its log, and stop its resource watchdog. This closing ends that plan's resume-log protocol and its review
   demands, as an express stop; the next plan starts its own. Then update the plans log.

To notify, use the harness's own notification facility (in Claude Code, the push notification tool). If there is none,
say so in the flow's final report instead.

When a blocked plan becomes eligible again, its remaining work stacks at the tip of the plan stack at that point,
resuming from its own log. The user answers a blocked plan's question either in the executor's own session, which then
records the answer in the plans log and in the plan's own log, or by revising the plan file on main, which step 3
notices through the blob hash. No other session writes either log.

If a push is refused mid-plan because a bookmark moved on the remote, the user probably landed or edited part of the
stack while the plan ran. Reconcile as in step 1 before continuing.

## Draining: "drain the plans"

Execute the next plan, then the next, until a round finds nothing eligible. Finish by reporting what was built, with PR
links, and which plans are blocked on what.

"Drain the plans and keep monitoring" adds a loop around that. When a drain finishes, record the next wake time, sleep
one hour, then drain again, and keep going until the user says stop. Each round fetches, but rebases only when it is
about to run a plan: a round with nothing eligible does not rebase or push anything, so an idle monitor does not
force-push every open plan PR every hour. Sleep in a way that wakes the session without polling: in Claude Code, a
background `sleep 3600` run with a background timeout longer than the sleep, whose completion re-invokes the session.
The point is that a user can leave one agent monitoring and add plans from other sessions whenever they like, without
coordinating with it.

## Reports: "show the plan reports"

When a plan closes (complete or blocked), its final report goes to the user in chat exactly as a goal's final report
normally would, and a verbatim copy goes to `farhelm-plan-<slug>-report.md`, replacing any earlier copy. That copy is
what lets the user review a stack of finished plans later, after the chat that produced them has scrolled away or ended.

"Show the plan reports", "show me the final reports", or similar means: print the report of every plan in the plans log
that still has an open PR on GitHub, without writing the plans log. Order them by where each plan's lowest open PR sits
in the chain, bottom first, which is the order they would land in. Put each under a heading with its slug and its PR
numbers, and say so when a plan's PRs are split across more than one place in the chain. Reports for plans that have
landed stay on disk but are not listed.

## Cleanup: "delete the executed plans"

Executed plans are kept on main until the user says to remove them, so a finished plan can still be read next to the
code it produced. "Delete the executed plans" or similar means: on top of the latest `main@origin`, delete every plan
file whose `INDEX.md` line is `[executed]`, and those lines, as one commit and PR per `jjstack`. Validate with
`dprint check`, ask the user to approve, and merge once they do. An `[executed]` line on main already means the plan's
work has landed, so nothing else needs checking. Pending plans are never touched.

## Help: "plan help"

"Plan help", "help on plans", or similar means: summarize the planning, triage-scheduling, execute-next, drain,
drain-and-monitor, show-reports, and cleanup flows in no more than one to three lines each, then stop. Change nothing.

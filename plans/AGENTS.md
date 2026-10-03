# plans/ rules

This directory holds a queue of planned work. A plan is a goal file written by the `scode-build-goal` skill, for one
TODO.md entry or one triaged review outcome (occasionally a few very closely related ones), and executing a plan means
running it the way `/goal` would. Any number of executors work through the queue at once, each in its own checkout and
each holding one plan at a time. A finished plan's PRs stay open until the maintainer has been walked through its report
and approved it; only then does it land.

NOTE: A plan is not a design document or a spec. It is the instructions for an unattended run that builds the work as a
stack of PRs. SPEC.md and SPEC_impl.md stay authoritative over anything a plan says.

## Layout

- `queue/INDEX.md` lists every plan, one physical line each, in queue order (oldest first, unless the maintainer placed
  a plan elsewhere), with its state. The file is excluded from dprint, so a line is never rewrapped. Queue order is a
  picking preference, not a sequence (see the note under Planning).
- `queue/<slug>.md` is one plan, named briefly after the work (`create-off-read-loop.md`, `font-size-shortcuts.md`),
  never a number or a date, and with no project prefix. Every `.md` file in `queue/` other than `INDEX.md` is a plan.
- `reports/<slug>.report.md` is the report a finished plan delivered for review.

Every plan has a source that references it, so nobody plans or executes the same work twice: a TODO.md entry ends with
``Plan: `plans/queue/<slug>.md`.``, or a `TRIAGE_OUTCOMES.md` entry's Execution field reads
`` planned in `plans/queue/<slug>.md` ``. Having a plan does not move an entry between TODO.md buckets; in particular it
has nothing to do with the `Planned` bucket, which means something else (see TODO.md's own header).

## States, and the script that moves them

`INDEX.md` lines have this exact grammar:

```
- [<state>] `<slug>.md` — <summary>[ (after `<other>.md`[, `<other2>.md`])]
```

| State               | Meaning                                                              | Held by               |
| ------------------- | -------------------------------------------------------------------- | --------------------- |
| `pending`           | waiting for an executor: new, answered, or sent back for a follow-up | nobody                |
| `in-flight <claim>` | an executor is working on it                                         | that claim's executor |
| `blocked`           | waiting on the maintainer; the plan file has a `## Blocked` section  | nobody                |
| `in review`         | every PR built and gated; `reports/<slug>.report.md` delivered       | nobody                |
| `approved`          | the maintainer approved it; waiting to land                          | nobody                |

`(after x.md)` means the plan may only start once `x.md`'s work is on main. That is the case once `x.md` is no longer
listed at all: only landing (`remove`) and abandoning take a line out, and abandoning settles its dependents explicitly.

Every state change is made by `scripts/plans-queue.py`, which commits it directly to main. Below, `queue <verb>` is
short for `scripts/plans-queue.py --repo <OWNER/NAME> <verb>`, where `<OWNER/NAME>` is the GitHub repository of the
`origin` remote (the script needs it spelled out, because a jj workspace has no `.git` for `gh` to infer it from). The
script builds its commit on the main it just read and publishes it with a non-forced update that GitHub refuses if main
moved in between, so of two executors claiming the same plan exactly one wins and the other gets exit 10. Its header
documents the rest; in short: exit 0 done, 1 `check` found violations (or `check-slug` found the slug), 10 the queue is
not in the state the verb needs (look again and re-decide), 2 a usage mistake, 3 an error whose outcome may be unknown.

Exit 3 from a verb that moves a state does not mean nothing happened: the commit may have landed with its response lost.
Before doing anything else, run `queue status`. If the line already shows the verb's target state (for a claim holder,
with your own claim id where the state has one; `queue history <slug>` lists the plan's recent commits and the claim id
each carried), the verb landed. If it still shows the starting state, run the verb again. Anything else is someone
else's change: handle it as exit 10.

| Verb                                            | From → to                | Used by                                    |
| ----------------------------------------------- | ------------------------ | ------------------------------------------ |
| `claim <slug> --claim ID`                       | pending → in-flight      | an executor that picked the plan           |
| `unclaim <slug> --claim ID`                     | in-flight → pending      | the claim holder, giving the plan back     |
| `block <slug> --claim ID --question FILE`       | in-flight → blocked      | the claim holder                           |
| `deliver <slug> --claim ID --report FILE`       | in-flight → in review    | the claim holder                           |
| `release <slug>`                                | in-flight → pending      | only when the maintainer asks              |
| `answer <slug> --decision FILE`                 | blocked → pending        | a review session                           |
| `follow-up <slug> --decision FILE`              | in review → pending      | a review session                           |
| `approve <slug>`                                | in review → approved     | a review session                           |
| `remove <slug> --merged N[,N...]`               | approved → gone          | a landing session, once the PRs landed     |
| `abandon <slug> [--dependents block\|drop-dep]` | any but in-flight → gone | a review session, on the maintainer's word |

The read-only verbs: `status` (the queue with claims, claim times, open PRs, waiting dependencies, and flags for what a
person should look at; it prints main's commit id first), `check` (the invariants of a local tree, or of a remote ref
with `--ref`; with `--base <branch>`, also what a planning PR may not change), `check-slug <slug>`, `history <slug>`
(the plan's recent queue commits, each with the claim id it carried), and `wake-check` (used by the watcher).

The rules that follow from this:

- Never hand-edit a state, a `## Blocked` section, a `## Decisions` entry, or a report, and never force push main.
- Planning PRs are the only other writers of `plans/`. They may add plan files with new `[pending]` lines, reorder
  lines, reword a summary, and revise the file of a plan that is `pending` or `blocked`; nothing else.
  `queue check --base main` enforces that before a planning PR lands.
- Code PRs never touch `plans/`.
- `## Decisions` entries are part of the plan's goal: an entry wins over anything in the plan body it contradicts.

## Files outside the repository

Executors keep their state in the parent directory of the checkout, next to it and out of the repository, the way
`scode-build-goal` places goal logs (derive the parent the way its Placing the files section does). If that derivation
does not yield a usable directory, stop and tell the maintainer rather than picking a temporary location. Every executor
runs in its own sibling checkout or jj workspace on the same host, so they all share this directory; that is how a plan
started by one executor can be resumed by another. Separate clones are simplest. Workspaces of one jj repository also
work, since plan bookmarks never collide across plans, but they share bookmark state.

- `farhelm-plans-log-<checkout>.md`, where `<checkout>` is the checkout directory's name: the executor's own state, read
  first after any start, compaction or resume. It records the flow in progress; the current claim's slug and claim id;
  the verb it is about to run or last ran on that claim (`claiming`, `blocking`, `delivering`, `unclaiming`); the
  watcher's task handle and baseline commit while monitoring; and the reasoning behind each pick. Only that checkout's
  executor writes it.
- `farhelm-plan-<slug>-log.md`: the plan's working log, per the goal file's resume protocol. Whoever holds the plan's
  claim writes it; a later claimer reads all of it. Besides the goal's own entries it records the executing checkout's
  absolute path and, after each push, the commit pushed for each bookmark. Slugs are never reused, so an old log can
  never be mistaken for a new plan's.

These files are never committed, so absolute paths in them are fine. Nothing else about a plan lives outside the
repository.

## Public-repo hygiene

Blocked sections, decisions and reports land on main directly, with no PR review, in a repository that is or may become
public. Never put absolute paths, host names, user names, scratch directories, or raw log excerpts in them. Name a
plan's working log by its rule (`farhelm-plan-<slug>-log.md` beside the checkouts), never by path. Report checks as
redacted commands and recorder run ids, per root `AGENTS.md`. The script refuses text containing a home, temp or
`/private/` path or the host name, and names the line; rephrase and run it again. When that text is the maintainer's own
words, agree the rephrasing with them.

## Sub-agents this file requires

Three checks below run as fresh-context, read-only native sub-agents of the executing harness, on the executing
session's own model, writing their findings to a file in the executor's scratch directory: the resume check, and the
cold reads of a Blocked question and of a report. Every plan needs them, so they are exempt from a plan file's own rule
against delegating its work, and they are not routed through galaxy-brain.

## Planning: "plan to implement <TODO items>"

That request, or anything like it, starts the planning flow. Each named TODO entry gets its own plan, including when the
request identifies entries in bulk ("all the items in the near term bucket"). When some entries are very closely
related, you may propose merging them into one plan, but planning each entry alone is the default and merging needs the
maintainer's agreement. A merged plan references every entry it covers, and each of those entries references the plan.

Fetch first, and work from the latest `main@origin`. An entry that already references a plan is skipped and reported,
not planned again; a request to revise an existing plan updates that plan's file in place, which is allowed only while
the plan is `pending` or `blocked`. Never reuse a slug that has ever existed under `plans/`: `queue check-slug <slug>`
refuses one, because an old plan's working log may still sit beside the checkouts and a new plan with that name would
resume from it.

For each plan, run `scode-build-goal` with the entry's text (and whatever the maintainer said about it) as the goal,
with these overrides on top of the skill's own rules:

- No-workhorse mode.
- In the review gate menu, add a fresh-context Opus 5.5 agent at high effort with the skill's general review charter, as
  a pinned-model option, and present it first, as the default. The goal file records the choice as a demand for that
  model and effort, like any pinned option.
- The goal file is `plans/queue/<slug>.md` in this checkout, not the parent-directory default.
- The goal file contains no absolute paths: the repository may become public, and the plan will likely be executed from
  a different checkout than the one that wrote it. Refer to the repository as "the checkout the executing session runs
  in" and to everything in it by repo-relative path. Paths outside the repository are stated as rules (Files outside the
  repository above), and the executing session resolves them before it starts.
- The log is `farhelm-plan-<slug>-log.md` in the parent directory of the executing checkout, stated in the goal file as
  that rule. Its existence keeps the goal self-resumeable the way the skill intends.
- The delegation paragraph names the sub-agents this file requires (above) as exempt from the no-delegation demand.
- Stack and PRs: the stack's base is `main@origin`, or the plan's own open PRs when it resumes, never another plan's
  PRs; set up per Executing one plan below. Bookmarks are `plan/<slug>/<nn>-<short-name>`. PRs stay drafts; the executor
  never marks one ready and never merges. Within a run, a PR that needs correcting is restructured rather than corrected
  on top, and that applies to all of the plan's own open PRs, including ones an earlier run built.
- The plan never edits `plans/`: its state moves only through the script, as Executing one plan describes.
- A TODO plan's last code PR removes the TODO entries it covers.
- The done criterion: every PR exists and has passed its review gate, and the latest `## Decisions` entry, if any, is
  satisfied. Delivering the report is the executor's closing step, not part of the goal.
- The unattended fallback blocks per Executing one plan (Blocking).
- If the plan depends on another plan's work being on main first, the goal file says which, and so does its `INDEX.md`
  line.

New plans go at the end of `INDEX.md` unless the maintainer places them elsewhere.

NOTE: Queue order is not execution order. Several executors drain the queue at once, so any eligible plan may run at the
same time as any other, and a plan listed later can start, deliver and land before one listed earlier (Picking prefers
the oldest eligible plan, but skips it when it would conflict with unlanded work). Listing one plan below another orders
nothing. The only ordering between plans is `(after x.md)`, and that holds the dependent plan back until `x.md` has been
reviewed, approved and landed, so it waits on the maintainer. When pieces of work must be built in a particular order,
or one of them touches much of what the others touch (a broad rename that every other change would rebase across, say),
keep them in one plan whose goal builds them as one stack in that order; for separate TODO entries that is a merge to
propose, as above. Do not split such work into separate plans on the assumption that a later plan runs after an earlier
one: without a dependency they may run concurrently and conflict, and with one the later work idles behind review.

When every plan in the request is written, land them as one commit and one PR per the `jjstack` skill, on top of the
latest `main@origin`: the plan files, their `INDEX.md` lines, and the TODO.md references, all together. Validate with
`dprint check` on the changed files, `queue check`, and `queue check --base main` after the final rebase. A `--base`
violation on a line or report the PR did not touch means main moved under the check (another executor claimed or
delivered): fetch, rebase, and run it again. Ask the maintainer to approve, and merge per `jjstack` once they do. If the
merge is refused because main moved, rebase, check again, and retry. A plan that only lives in a working copy does not
exist as far as an executor is concerned, because executors only read plans from main.

## Planning triage outcomes: "use the planning system to schedule these"

Executing triage outcomes normally does not involve `plans/` at all: "execute triage outcomes", or turning them into a
goal with `scode-build-goal` directly, follows root `AGENTS.md` (Execute triage outcomes) and the skill as usual. This
section applies only when the maintainer explicitly asks for the planning system, for example "use the planning system
to schedule these for execution".

Such a request makes one plan per outcome ("these" after a triage session means the outcomes decided in it; ask if it is
unclear which). Grouping outcomes that are extremely closely related into one plan is something to propose, and needs
the maintainer's agreement, as in the TODO flow. Each plan's goal is to execute its outcome per root `AGENTS.md`'s
Execute triage outcomes (one PR per outcome) and names the outcome by its `TRIAGE_OUTCOMES.md` heading. Only outcomes
whose Execution is `pending` can be scheduled; skip and report the rest.

Plan them as in the TODO flow above, with the same `scode-build-goal` overrides, except:

- The decisions are already recorded in `TRIAGE_OUTCOMES.md`, so the up-front questions cover only what the ledger
  leaves open for unattended execution, not the outcomes themselves.
- There are no TODO entries to remove; the outcome's own PR records its execution in the ledger, as Execute triage
  outcomes requires.
- The planning PR carries the plan files, their `INDEX.md` lines, and the `` planned in `plans/queue/<slug>.md` ``
  Execution update for each scheduled outcome, in place of TODO references.

## Picking: "pick a plan to execute", "execute the next plan"

Those requests, and "use the planning system and pick one item to execute", mean: pick one plan by the judgment below,
then execute it. "Execute the next plan in order" takes the first eligible plan regardless of conflicts. "Execute plan
X" takes X if it is eligible and otherwise says why (in flight, blocked, waiting on a dependency) and stops; it does not
wait.

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
   - `unclaiming`: your id still on the line means the `unclaim` never landed; run it again. Otherwise it did.
   - `blocking` or `delivering`: if `history` shows that commit with your claim id, it landed, whatever the line says
     now (review may already have moved on); finish closing (step 12). If not and your id is still on the line, run the
     verb again. Otherwise the claim was released.

   A released claim means: drop it from the executor log, push nothing more for that plan and write nothing more to its
   working log, notify, and continue with a fresh pick.
2. `jj git fetch`, `queue status`, and pick (Picking above). Nothing picked: the round is over.
3. Claim. Generate a claim id (`python3 -c 'import secrets; print(secrets.token_hex(3))'`), write slug, id and
   `claiming` to the executor log, then `queue claim <slug> --claim <id>`. Exit 10 means someone else got there first:
   pick again, reusing the analysis you already have for the remaining candidates. Exit 3 is ambiguous: run
   `queue status`; if the line carries your id, the claim landed. Otherwise leave `claiming (uncertain)` in the executor
   log and pick again; step 1 settles it on the next start, since a late landing is still possible.
4. Re-check. Run `queue status` again. If a plan claimed since your pick is a strong conflict with this one, give the
   claim back (`queue unclaim`) and pick again.
5. Read the plan file from `main@origin`, including any `## Decisions`.
6. Set up the base. If the plan has open `plan/<slug>/*` PRs (a resume), track those bookmarks, rebase that stack onto
   `main@origin` as a careful rebase (root `AGENTS.md`), and build on its tip; otherwise start a new change on
   `main@origin`. Conflicts in the shared bookkeeping files listed under Picking are mechanical: keep both sides. A
   conflict that needs a design decision is a block (step 10). A plan PR the maintainer closed without merging is the
   maintainer rejecting it, unless a Decisions entry says otherwise: block with that as the question rather than
   rebuilding it or carrying on above it.
7. Resume check, whenever the plan has a working log or open PRs. Read the whole working log, the open PRs, and the
   Decisions, then write a "where this stands" entry in the working log: what is built and pushed, what is half done,
   what the latest decision asks for, and what comes next. If the log names another checkout and its last recorded
   pushes differ from GitHub's, inspect that checkout without changing it (`jj --ignore-working-copy -R <path> log`);
   unpushed work found there is a block, with that as the question, rather than something to rebuild blind. A sub-agent
   then checks the entry against the same sources. Reconcile any disagreement before working. If the plan cannot be
   reconciled, give the claim back and notify. Always start your own resource watchdog; treat the previous executor's
   scratch directory as read-only input. After a release, entries the released executor wrote to the working log before
   it noticed are expected: record that you saw them and continue, rather than stopping as the resume protocol would for
   entries of unknown origin.
8. Run the plan file as the goal. For a single plan, set the harness's goal to it if the harness lets the model set its
   own goal; otherwise, and always within a drain, behave exactly as if it were the harness-provided goal under the
   outer request. A drain does not register each plan as a harness goal, because a plan that blocks could not be cleared
   to make room for the next one.
9. Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id. If it does
   not, stop pushing and treat the claim as released (step 1). After each push, record the pushed commits in the working
   log.
10. Blocking: when the remaining work waits on a decision only the maintainer can make (the goal file's unattended
    fallback), push everything that is consistent, then write the question for the Blocked section (Writing for the
    maintainer below). Have a sub-agent cold-read it, asking whether the maintainer could decide from this text alone
    and whether it breaks public-repo hygiene, and revise until it passes. Write `blocking` to the executor log, run
    `queue block <slug> --claim <id> --question <file>`, notify, and close (step 12).
11. Finishing: when the done criterion holds, rebase the stack onto `main@origin` as a careful rebase if main has moved
    in a way that touches it, so the report describes what would actually land. Then write the report (Writing for the
    maintainer below) and have a sub-agent cold-read it, asking whether the maintainer could approve or ask for a
    follow-up from it alone and whether it breaks public-repo hygiene. Revise until it passes, write `delivering` to the
    executor log, run `queue deliver <slug> --claim <id> --report <file>`, and notify.
12. Close: write a closing entry in the plan's working log, stop its resource watchdog, and clear the claim from the
    executor log. Closing ends the plan's resume-log protocol and its review demands, as an express stop; the next plan
    starts its own.

`block` or `deliver` exiting 10 means the claim was released while you worked: push nothing more, record it in both
logs, and notify. To give a claim back for any other reason (the maintainer stops the flow mid-plan, a `gh` login that
expired, a disk alert that cannot be cleared), push what is consistent, write the working log, write `unclaiming` to the
executor log, run `queue unclaim`, and notify.

To notify, use the harness's own notification facility (in Claude Code, the push notification tool). If there is none,
say so in the flow's final report instead.

### Writing for the maintainer

The maintainer reads Blocked questions and reports without opening the code, without remembering what a PR number stands
for, and without remembering the review finding or triage decision behind the plan. Every decision a report records and
every question it asks must be one the maintainer can take a position on straight away. In product terms, per root
`AGENTS.md` "Talking to the user", state:

- the feature or operation involved and the original problem: what went wrong for a user, under what trigger, and what
  they saw or lost;
- what the maintainer decided about it during triage or in earlier Decisions, if anything;
- what the plan did, and what execution found that the decision did not anticipate;
- for a question: the question itself, the realistic options with their tradeoffs, and a recommendation.

A PR number may follow as a pointer, never in place of that context. Use `###` or deeper headings in a Blocked question;
the script refuses `#` and `##`, which would break the plan file's sections.

A report has these sections, in this order: what this was about; things you should know; open questions and possible
follow-ups; the PRs, one line each; checks run, reused and skipped, with the reason for each; the review gate's outcome.
In a later round, a "since the last review" section comes first and names which PRs changed, were added, or were
dropped.

## Draining: "drain the plans"

Execute one plan after another until a pick takes nothing. Finish by reporting what was delivered and what blocked, with
PR links, restating any open questions the way Writing for the maintainer requires.

Any number of executors may drain at once, each in its own checkout; claims keep them off each other's plans.

"Drain the plans and keep monitoring" adds a loop around that: when a drain finishes, wait until there may be new work,
drain again, and keep going until the maintainer says stop. The waiting is done by `scripts/plans-watch.sh`, not by the
model. A model wake-up re-reads the agent's whole conversation, uncached after any long wait, so waking hourly just to
find nothing would cost a full model round each time. The watcher polls GitHub cheaply and exits only when `plans/` on
main changes in a way that can give an idle executor work: with `--wake-check`, claims alone do not count. An idle
executor still wakes when the watcher reaches its maximum wait, about every 110 minutes by default, but that wake-up
only restarts the watcher. Run it like this:

- Start
  `scripts/plans-watch.sh --repo <OWNER/NAME> --baseline-from <commit> --wake-check <checkout>/scripts/plans-queue.py`
  from the checkout, where `<commit>` is the main commit id `queue status` printed for the drain's last pick and
  `<checkout>` is the checkout's absolute path. Use that commit, not a fresh fetch, so a plan that landed in between
  still counts as a change. If that pick's `status` failed, do not start a watcher at all: notify the maintainer and
  stop monitoring.
- Run it as a background command whose completion wakes the session: in Claude Code, `run_in_background` with a timeout
  above the watcher's `--max-wait` (the defaults, 6600 seconds against Claude Code's two-hour background limit, fit).
  Record its task handle and the baseline commit in the executor log. After a compaction or a resume, if the recorded
  watcher is no longer running, start a new one with the recorded commit.
- Act only on the exit of the watcher the executor log records. Exits of a watcher the agent stopped itself, or of any
  other, are ignored.
- Its exit decides what happens next:
  - `changed` (status 0): drain again.
  - `idle` (status 10): nothing changed; start it again with the same commit, without fetching or draining.
  - `error:` (status 3): if the line mentions HTTP 401 or 403, the GitHub login needs the maintainer; notify and stop
    monitoring. Otherwise notify once, then keep restarting it with `--interval 1800` until a watcher exits `changed` or
    `idle`, which also restores the default interval. Do not notify again for the same run of errors.
  - `usage:` (status 2) or anything else: the invocation is wrong, so restarting cannot help. Notify and stop
    monitoring.
- A harness that cannot wake a session when a background command exits runs the watcher in the foreground instead, with
  a `--max-wait` shorter than its own command timeout.

A batch of new plans wakes every monitoring executor at once, and they will race for the same first pick. That is fine:
the losers get exit 10 and move to their next candidate.

"Check for plans", or similar, said to a monitoring agent (typically by interrupting it while it waits) means: do not
wait for the watcher. Stop it, drain now, and then start a new watcher from that drain's last pick, so only one watcher
ever runs.

## Reviewing the plans: "review the plans"

Run this in its own checkout, never in an executor's. It works through every plan waiting on the maintainer: first
`[in review]` plans in queue order, since landing finished work reduces how much is in flight at once, then `[blocked]`
ones. "Show blocked plans" is the same flow limited to blocked ones.

For each plan, start with plain-language context (root `AGENTS.md` "Talking to the user"): what the work is about, in
product terms. Then present the report or the Blocked question, and ask for a decision:

- In review:
  - approve: `queue approve`;
  - follow-up: the plan is not done; write the maintainer's words verbatim (plus any agreed restatement) to a file and
    run `queue follow-up --decision <file>`, which puts it back to `[pending]` with the follow-up in its Decisions;
  - abandon (below).
- Blocked:
  - answer: the maintainer's words verbatim (plus any agreed restatement) to `queue answer --decision <file>`, which
    moves the question and the answer into Decisions and puts the plan back to `[pending]`;
  - abandon (below).
- Either: leave it for later, which changes nothing.

Record each decision as it is made, so executors can pick up answered and followed-up plans while the review goes on.
End by listing the approved plans waiting to land; landing them is a separate request, usually at the end of the same
session.

Abandoning a plan: close its open PRs, then run `queue abandon`. If other plans run after it, ask the maintainer whether
they block (`--dependents block`, which gives each one a Blocked question about it) or drop the dependency
(`--dependents drop-dep`). Ask what happens to the plan's source (the TODO entry goes back to unplanned, or the triage
outcome's Execution back to `pending`, or something else), and land that as its own PR per `jjstack`.

## Landing the approved plans: "land the approved plans"

Run this in its own checkout. Landing is never "merge, and see whether version control reports a conflict". Before
anything merges, review on purpose what has landed on main since each approved plan's stack was based, and what is about
to land together: the diffs, not the titles, read against each other. Look for interactions that produce no textual
conflict: something one change renamed or repurposed that another relies on, a contract a new caller assumes that
another change altered, a new caller of changed code, a changed invariant, lock or ordering, a SPEC.md or SPEC_impl.md
amendment that another change now contradicts. Root `AGENTS.md` "Careful rebase" lists the kinds; apply them across the
whole set, not one stack at a time. Have a fresh-context sub-agent do the same review independently and reconcile any
disagreement. Fix what is obvious as part of the landing; a conflict that needs a design decision stops the landing and
goes to the maintainer, and the plans stay approved.

Then, for each `[approved]` plan in queue order:

1. Fetch, and rebase its stack onto `main@origin` as a careful rebase. Conflicts in the shared bookkeeping files listed
   under Picking are mechanical: keep both sides. Main has moved since the review above if an earlier plan just landed;
   check what that plan changed against this one again.
2. Confirm no PR in the stack touches `plans/`.
3. Run the checks the rebase calls for, per root `AGENTS.md` "Finishing work".
4. Land the stack bottom-up per `jjstack`, marking each PR ready as it lands. Bookkeeping commits make main move often,
   so a merge refused because the base branch changed means re-read and retry, not stop.
5. `queue remove <slug> --merged <the PR numbers just landed>`, which refuses while any of the plan's PRs is still open
   and checks that each named PR merged.

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

`queue status` flags an approved plan with no open PRs, with the merged PR numbers it can find, so a landing that
stopped between the last merge and `remove` is finished by the next one. When it finds none merged (an interrupted
abandon, or PRs the maintainer closed), it says to ask the maintainer: never `remove` a plan whose work did not land. An
approved plan can only leave the queue by landing or by being abandoned; if the maintainer changes their mind before it
lands, abandon it and plan again.

## Releasing: "release plan X"

Only on the maintainer's request, for a plan whose executor died or is stuck: `queue release <slug>`. It goes back to
`[pending]`, and whoever claims it next resumes it from its working log and open PRs (Executing one plan, step 7). If
the old executor is in fact still alive, its next push check or `block`/`deliver` finds the claim gone and it stops.

## Help: "plan help"

"Plan help", "help on plans", or similar means: summarize each flow in this file in one to three lines, then stop.
Change nothing. The flows: plan to implement TODO items; schedule triage outcomes through the planning system; pick a
plan to execute (and the in-order and named variants); drain the plans; drain and keep monitoring, including check for
plans; review the plans, including show blocked plans; land the approved plans; release plan X; and abandoning, which
happens inside review.

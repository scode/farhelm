# plans/ rules

This directory holds a queue of planned work. A plan is a goal file written by the `scode-build-goal` skill, for one
TODO.md entry or one triaged review outcome (occasionally a few very closely related ones), and executing a plan means
running it the way `/goal` would. Any number of executors work through the queue at once, each in its own checkout and
each holding one plan at a time. A finished plan lands without waiting for the maintainer: one monitor merges what the
executors finish, and the maintainer reads each landed plan's report afterwards, then approves it or asks for a
follow-up. Only a blocked plan waits on the maintainer before anything moves.

NOTE: A plan is not a design document or a spec. It is the instructions for an unattended run that builds the work as a
stack of PRs. SPEC.md and SPEC_impl.md stay authoritative over anything a plan says.

## Layout

- `queue/INDEX.md` lists every plan, one physical line each, in queue order (oldest first, unless the maintainer placed
  a plan elsewhere), with its state. The file is excluded from dprint, so a line is never rewrapped. Queue order is a
  picking preference, not a sequence (see the note under Planning).
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

## States, and the script that moves them

`INDEX.md` lines have this exact grammar:

```
- [<state>] `<slug>.md` — <summary>[ (after `<other>.md`[, `<other2>.md`])]
```

| State               | Meaning                                                                       | Held by               |
| ------------------- | ----------------------------------------------------------------------------- | --------------------- |
| `pending`           | waiting for an executor: new, answered, or sent back for a follow-up          | nobody                |
| `in-flight <claim>` | an executor is working on it                                                  | that claim's executor |
| `blocked`           | waiting on the maintainer; the plan file has a `## Blocked` section           | nobody                |
| `complete`          | every PR built and gated; `reports/<slug>.report.md` delivered; ready to land | nobody                |
| `landing <claim>`   | the monitor is landing it                                                     | that claim's monitor  |

A plan that has landed is not in `INDEX.md` at all. Landing moves its line to `REPORTS.md`, which has this grammar:

```
- [`<slug>`](reports/<slug>.report.md) landed <YYYY-MM-DD> in #<n>[, #<n>...]: <summary>
```

It stays there until the maintainer approves it, which ends its tracking, or asks for a follow-up, which puts it back at
the top of `INDEX.md` as `[pending]`.

`(after x.md)` means the plan may only start once `x.md`'s work is on main. That is the case once `x.md` is no longer
listed in `INDEX.md`: only landing (`landed`) and abandoning take a line out, and abandoning settles its dependents
explicitly. A plan can therefore start as soon as what it waits for has landed, whether or not the maintainer has read
that plan's report yet. A follow-up does not take that back: when it puts `x.md` back into `INDEX.md`, the script drops
`x.md` from every `(after ...)` clause, because its first round is already on main. The maintainer chose that changes
keep flowing by default; if a follow-up would break a dependent, they deal with it by hand.

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

| Verb                                                      | From → to                               | Used by                                    |
| --------------------------------------------------------- | --------------------------------------- | ------------------------------------------ |
| `claim <slug> --claim ID`                                 | pending → in-flight                     | an executor that picked the plan           |
| `unclaim <slug> --claim ID`                               | in-flight → pending                     | the claim holder, giving the plan back     |
| `block <slug> --claim ID --question FILE`                 | in-flight or landing → blocked          | the claim holder                           |
| `deliver <slug> --claim ID --report FILE`                 | in-flight → complete                    | the claim holder                           |
| `start-landing <slug> --claim ID`                         | complete → landing                      | the monitor                                |
| `stop-landing <slug> --claim ID`                          | landing → complete                      | the claim holder, giving the landing back  |
| `landed <slug> --claim ID --merged N[,N...] --notes FILE` | landing → `REPORTS.md`                  | the claim holder, once every PR merged     |
| `release <slug>`                                          | in-flight → pending, landing → complete | only when the maintainer asks              |
| `answer <slug> --decision FILE`                           | blocked → pending                       | a review session                           |
| `follow-up <slug> --decision FILE`                        | `REPORTS.md` → pending, first in line   | a review session                           |
| `approve <slug>`                                          | `REPORTS.md` → gone                     | a review session                           |
| `abandon <slug> [--dependents block\|drop-dep]`           | pending, blocked or complete → gone     | a review session, on the maintainer's word |

The read-only verbs: `status` (the queue with claims, claim times, open PRs, waiting dependencies, and flags for what a
person or the monitor should look at, then the landed plans awaiting review; it prints main's commit id first), `check`
(the invariants of a local tree, or of a remote ref with `--ref`; with `--base <branch>`, also what a planning PR may
not change), `check-slug <slug>`, `history <slug>` (the plan's recent commits under `plans/`, each with the claim id it
carried), and `wake-check` (used by the watcher, with `--for executor` or `--for lander`).

The rules that follow from this:

- Never hand-edit a state, a `## Blocked` section, a `## Decisions` entry, a report, or `REPORTS.md`, and never force
  push main.
- Planning PRs are the only other writers of `plans/`. They may add plan files with new `[pending]` lines, reorder
  lines, reword a summary, and revise the file of a plan that is `pending` or `blocked`; nothing else, and in particular
  never `REPORTS.md`, a report, or a landed plan's file. `queue check --base main` enforces that before a planning PR
  lands.
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
- `farhelm-plans-monitor-log.md`: the monitor's own state, read first after any start, compaction or resume (Landing
  below). There is one, because one monitor runs at a time. It starts with a `## Status` section holding one line,
  `running, last checked <UTC time>` or `stopped <UTC time>: <reason>`, which the monitor rewrites on every wake and
  which a review session reads. Below that it records the landing claim in progress (slug, claim id, and the verb it is
  about to run or last ran: `starting`, `blocking`, `stopping`, `recording`), the PRs of that plan merged so far, the
  watcher's task handle and baseline commit, whether the maintainer chose to run without notifications, and the claim
  ids and plans it has already notified about. Only the monitor writes it.

These files are never committed, so absolute paths in them are fine. Nothing else about a plan lives outside the
repository.

## Public-repo hygiene

Blocked sections, decisions, reports, landing notes and `REPORTS.md` land on main directly, with no PR review, in a
repository that is or may become public. Never put absolute paths, host names, user names, scratch directories, or raw
log excerpts in them. Name a plan's working log by its rule (`farhelm-plan-<slug>-log.md` beside the checkouts), never
by path. Report checks as redacted commands and recorder run ids, per root `AGENTS.md`. The script refuses text
containing a home, temp or `/private/` path or the host name, and names the line; rephrase and run it again. When that
text is the maintainer's own words, agree the rephrasing with them.

## Notifications

Notifications go through [ntfy.sh](https://ntfy.sh/), to a topic the maintainer keeps in `~/.farhelm-ntfy-topic` on the
machine the agents run on. Anyone who knows an ntfy.sh topic can read what is sent to it, so the topic name is the only
thing keeping these notifications private. That is why it never goes into the repository, a log, a report, or the
conversation: never read, print, quote or log that file, not even to check its contents. Assume it holds a correct topic
name.

A notification means one thing: progress may be stopped until the maintainer acts. Send one for that and nothing else;
never for something the maintainer did themselves (releasing a claim, stopping a flow), for a delivery or a landing that
went through, for an error that is still being retried, or for anything else that is only for their information. The
agent that discovers the condition sends it: an executor for a plan it blocks or a problem that stops it (Executing one
plan and Draining below), the monitor for a landing it blocks or cannot finish, a problem that stops it, and what it
sees in `queue status` (Landing below). Each place below that says to notify is such a condition.

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

Whenever work on a plan ends, the executor ALWAYS returns to a clean working copy of the latest `main`, inside a drain
or not, and whether the plan delivered, blocked, or gave up its claim. Unlike the starting check, this one does not
abort on a dirty working copy: it cleans it up. When the executor closes a plan it still held (it delivered, blocked, or
gave the claim back itself after pushing), every consistent piece of the plan has been pushed (steps 9 to 11 under
Executing one plan), so anything still local is a leftover (a half-made edit, a scratch file jj snapshotted, an unpushed
or amended commit on the plan's stack), and leaving it in place is how the next plan ends up building on top of it. The
starting check is different because a working copy found dirty before any plan runs may be the maintainer's own work or
an interrupted executor's unpushed progress, and neither is the executor's to throw away.

Here's the reset after a close, run from the checkout, with `<slug>` the plan's slug:

```
jj git fetch
s='::@ & mutable() & ~::remote_bookmarks()'
jj diff --summary -r "($s) ~ ::(($s):: ~ ($s))"
jj abandon --retain-bookmarks -r "($s) ~ ::(($s):: ~ ($s))"
```

Then put each local `plan/<slug>/*` bookmark back where GitHub has it, and move onto `main`:

- a bookmark with an `@origin`: `jj bookmark set <name> -r <name>@origin --allow-backwards`;
- a bookmark that was never pushed: `jj bookmark delete <name>`;
- finally `jj new main@origin` and `jj status`, which must report no changes and no conflicted `plan/<slug>/*` bookmark.

The fetch comes first because `remote_bookmarks()` is only as current as the last fetch, and a commit that reached
GitHub since then must not count as unpushed. `$s` is the working copy and its ancestors that no remote bookmark
reaches; the `~ ::(($s):: ~ ($s))` part leaves out any of those that something outside the set builds on (another jj
workspace's working copy, another plan's commit), because abandoning them would rewrite that history under its owner.
What the guard leaves out stays where it is, outside the working copy. The `jj diff` lists the paths being dropped, and
the plan's closing log entry records them so a discard is never silent. The bookmark step is needed because
`--retain-bookmarks` only moves a bookmark to the abandoned commit's parent: that is its pushed commit when the local
work merely extended the stack, but `main` when the pushed commit was amended or rebased, and a later push of that name
would then empty the PR. Without `--retain-bookmarks` the abandon deletes the bookmark outright, which is worse.

When the claim was released rather than given up (step 1, or `block`/`deliver` exiting 10), the executor no longer owns
the plan and the reset is narrower: `jj git fetch`, then `jj new main@origin` and `jj status`, with nothing abandoned
and no bookmark touched. Unpushed work may still be local at that point (step 9 forbids pushing it once the claim is
gone), and step 7 has the next claimer inspect exactly that work in this checkout before deciding what to do with it.
Moving off it keeps the working copy clean without destroying it; record the old working-copy commit id in the executor
log so the leftover can be found.

Either reset leaves ignored files alone, since jj never sees them. That keeps the checkout's build caches (`target/`,
`node_modules/`, `.ci-tmux/`, and the like) for the next plan, and it also leaves the other ignored files (release
drafts, delegation records, saved authentication state) where their owners expect them. A monitor's baseline commit is
not a reason to skip the reset.

## Sub-agents this file requires

Three checks below run as fresh-context, read-only native sub-agents of the executing harness, on the executing
session's own model, writing their findings to a file in the executor's scratch directory: the resume check, and the
cold reads of a Blocked question and of a report. Every plan needs them, so they are exempt from a plan file's own rule
against delegating its work, and they are not routed through galaxy-brain. The monitor uses sub-agents the same way for
its independent review of what is about to land and for the cold read of a Blocked question it writes.

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
nothing. The only ordering between plans is `(after x.md)`, and that holds the dependent plan back until the monitor has
landed `x.md`, which waits on the maintainer only if `x.md` or its landing blocks. When pieces of work must be built in
a particular order, or one of them touches much of what the others touch (a broad rename that every other change would
rebase across, say), keep them in one plan whose goal builds them as one stack in that order; for separate TODO entries
that is a merge to propose, as above. Do not split such work into separate plans on the assumption that a later plan
runs after an earlier one: without a dependency they may run concurrently and conflict, and with one the later work
idles until the earlier work lands.

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
   - `unclaiming`: your id still on the line means the `unclaim` never landed; run it again. Once it has landed, finish
     the close it was part of: step 12 if the plan is still unclaimed, or, if another executor has claimed it since, the
     reset for a released claim (Clean-main boundary), recorded in the executor log only.
   - `blocking` or `delivering`: if `history` shows that commit with your claim id, it landed, whatever the line says
     now (the monitor or a review may already have moved on); finish closing (step 12). For `blocking`, also notify: the
     crash may have come between the block and its notification, and a second notification costs less than none. If not
     and your id is still on the line, run the verb again. Otherwise the claim was released.

   A released claim means: drop it from the executor log, push nothing more for that plan and write nothing more to its
   working log, run the reset for a released claim (Clean-main boundary), and continue with a fresh pick. A release is
   the maintainer's own doing, so it is not worth a notification.
2. Apply the clean-main boundary, then run `queue status` and pick (Picking above). Nothing picked: the round is over.
3. Claim. Generate a claim id (`python3 -c 'import secrets; print(secrets.token_hex(3))'`), write slug, id and
   `claiming` to the executor log, then `queue claim <slug> --claim <id>`. Exit 10 means someone else got there first:
   pick again, reusing the analysis you already have for the remaining candidates. Exit 3 is ambiguous: run
   `queue status`; if the line carries your id, the claim landed. Otherwise leave `claiming (uncertain)` in the executor
   log and pick again; step 1 settles it on the next start, since a late landing is still possible.
4. Re-check. Run `queue status` again. If a plan claimed since your pick is a strong conflict with this one, give the
   claim back (`queue unclaim`) and pick again from step 2, whose boundary brings the checkout up to the `main` the
   unclaim just moved.
5. Read the plan file from `main@origin`, including any `## Decisions`.
6. Set up the base from the clean `main@origin` working copy. If the plan has open `plan/<slug>/*` PRs (a resume), track
   those bookmarks, rebase that stack onto `main@origin` as a careful rebase (root `AGENTS.md`), and build on its tip;
   otherwise start a new change on `main@origin`. Conflicts in the shared bookkeeping files listed under Picking are
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
    fallback), push everything that is consistent, then write the question for the Blocked section (Writing for the
    maintainer below). Have a sub-agent cold-read it, asking whether the maintainer could decide from this text alone
    and whether it breaks public-repo hygiene, and revise until it passes. Write `blocking` to the executor log, run
    `queue block <slug> --claim <id> --question <file>`, notify (Notifications above), and close (step 12).
11. Finishing: when the done criterion holds, rebase the stack onto `main@origin` as a careful rebase if main has moved
    in a way that touches it, so the report describes what would actually land. Then write the report (Writing for the
    maintainer below) and have a sub-agent cold-read it, asking whether the maintainer could approve or ask for a
    follow-up from it alone and whether it breaks public-repo hygiene. Revise until it passes, write `delivering` to the
    executor log, and run `queue deliver <slug> --claim <id> --report <file>`. The plan is now `[complete]`, and the
    monitor lands it from there.
12. Close: return to clean `main` with the reset under Clean-main boundary, write a closing entry in the plan's working
    log that names any paths the reset discarded, stop its resource watchdog, and clear the claim from the executor log.
    Closing ends the plan's resume-log protocol and its review demands, as an express stop; the next plan starts its
    own.

`block` or `deliver` exiting 10 means the claim was released while you worked: push nothing more, record it in both
logs, and run the reset for a released claim (Clean-main boundary). To give a claim back for any other reason (the
maintainer stops the flow mid-plan, a `gh` login that expired, a disk alert that cannot be cleared), push what is
consistent, write the working log, write `unclaiming` to the executor log, run `queue unclaim`, and close (step 12).
Notify unless the maintainer asked for the stop: an expired login or a full disk stops every plan on the machine until
someone fixes it, and a plan that could not be reconciled (step 7) will stop the next executor the same way. If pushing
the consistent work failed (the expired login, say), close with the reset for a released claim instead of the full one,
so that work stays in the checkout for the next claimer to find (step 7), and say so in the working log and the
notification.

To notify, follow Notifications above. Delivering does not notify: the monitor lands the plan, and its report reaches
the maintainer through `REPORTS.md`. An executor that dies cannot notify at all; its claim shows up in `queue status` as
possibly abandoned, and the monitor notifies about that (Landing below).

### Writing for the maintainer

The maintainer reads Blocked questions and reports without opening the code, without remembering what a PR number stands
for, and without remembering the review finding or triage decision behind the plan. A report is read after its work has
landed, so it is the maintainer's only account of what reached main and what they should know about it. Every decision a
report records and every question it asks must be one the maintainer can take a position on straight away. In product
terms, per root `AGENTS.md` "Talking to the user", state:

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

Start with step 1 of Executing one plan, which settles any claim or close that an earlier run of this executor left
unfinished; a dirty working copy that belongs to such a claim is resumed or reset there, not reported. Then get onto a
clean, freshly fetched `main` with the clean-main boundary's starting check, even when the working copy already looks
clean: an older `main` is not good enough. If that check finds a dirty working copy, abort the drain and tell the
maintainer which paths are dirty. Then execute one plan after another, until a pick takes nothing. Each plan's close
resets the working copy to a clean, fresh `main` (step 12), so the next pick always starts there. Every "drain again"
under monitoring below starts with the same starting check. Finish by reporting what was delivered and what blocked,
with PR links, restating any open questions the way Writing for the maintainer requires.

Any number of executors may drain at once, each in its own checkout; claims keep them off each other's plans.

"Drain the plans and keep monitoring" adds a loop around that: when a drain finishes, wait until there may be new work,
drain again, and keep going until the maintainer says stop. The waiting is done by `scripts/plans-watch.sh`, not by the
model. A model wake-up re-reads the agent's whole conversation, uncached after any long wait, so waking hourly just to
find nothing would cost a full model round each time. The watcher polls GitHub cheaply and exits only when `plans/` on
main changes in a way that can give an idle executor work: with `--wake-check`, only a plan becoming pickable (new,
answered, followed up, given back, revised, reordered, or freed by a dropped dependency), a plan leaving the queue, or
an in-flight plan blocking (which can end a conflict a pick skipped for) counts, and claims, deliveries, landings in
progress and report reviews do not. An idle executor still wakes when the watcher reaches its maximum wait, about every
110 minutes by default, but that wake-up only restarts the watcher. Run it like this:

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
    monitoring. Otherwise keep restarting it with `--interval 1800` until a watcher exits `changed` or `idle`, which
    also restores the default interval. An outage that is still being retried does not need the maintainer, so it is
    recorded in the executor log, not notified.
  - `usage:` (status 2) or anything else: the invocation is wrong, so restarting cannot help. Notify and stop
    monitoring.
- A harness that cannot wake a session when a background command exits runs the watcher in the foreground instead, with
  a `--max-wait` shorter than its own command timeout.

A batch of new plans wakes every monitoring executor at once, and they will race for the same first pick. That is fine:
the losers get exit 10 and move to their next candidate.

## Reviewing the plans: "review the plans"

NOTE: This is review after the fact. By the time a plan reaches this flow its work is already on main; approving it ends
its tracking, and it does not gate anything. The one exception is a blocked plan, which waits for an answer.

Run this in its own checkout, never in an executor's or the monitor's. It starts with the monitor's status, then works
through every landed plan in `REPORTS.md`, oldest first, then every `[blocked]` plan in queue order. "Show blocked
plans" and "check for blocked plans" are the same flow limited to blocked plans, still starting with the monitor's
status.

The monitor's status is one line, taken from the `## Status` section of `farhelm-plans-monitor-log.md` beside the
checkouts: when it last checked, or when and why it stopped. An idle monitor checks in at least every 110 minutes, so a
`running` line whose last check is more than about two and a half hours old means it has probably died; say so. A
missing log means no monitor has run on this machine, and nothing is landing. Add any flags `queue status` raises.

For each landed plan, start with plain-language context (root `AGENTS.md` "Talking to the user"): what the work was
about, in product terms. Then walk the maintainer through the report rather than pasting it: what landed, the things
they should know, the open questions and possible follow-ups, anything the Landing section adds (what it landed
alongside, fixes made while landing), and a link to the full report on GitHub,
`https://github.com/<OWNER/NAME>/blob/main/plans/reports/<slug>.report.md`. Then ask for a decision:

- approve: `queue approve`, which removes the plan's `REPORTS.md` line, its plan file and its report. The work stays on
  main; only its tracking ends.
- follow-up: the plan needs another round on top of what landed (a fix, a missing piece, a revert). Write the
  maintainer's words verbatim (plus any agreed restatement) to a file and run `queue follow-up --decision <file>`, which
  puts the plan back at the top of `INDEX.md` as `[pending]` with the follow-up in its Decisions. Its report stays, for
  the next round's "since the last review" section. Plans that ran after it no longer wait on it (States above).
- something that deserves its own plan or a TODO.md entry: that is a separate request handled the usual way (planning,
  or a TODO edit as root `AGENTS.md` describes), after which this plan can be approved.
- leave it for later, which changes nothing.

For each blocked plan, start with the same context, then present the Blocked question, and ask for a decision:

- answer: the maintainer's words verbatim (plus any agreed restatement) to `queue answer --decision <file>`, which moves
  the question and the answer into Decisions and puts the plan back to `[pending]`;
- abandon (below);
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

## Landing: "monitor for complete plans", "land the complete plans"

NOTE: Landing does not wait for the maintainer. Executors deliver finished plans as `[complete]`, the monitor lands
them, and the maintainer reads the reports afterwards (Reviewing the plans above). That makes the monitor's own review
of what it lands the last check before main, so it is done on purpose, not by letting version control report conflicts.

"Monitor for complete plans" runs landing rounds until the maintainer says stop, waiting in between. "Land the complete
plans" is one round, then stop. Run either in its own checkout, and only one at a time: landing claims keep two monitors
from landing the same plan, but each would review what it is about to land without seeing what the other lands.

### Starting and resuming

Read the monitor log first (Files outside the repository). Check for the ntfy topic file (Notifications above), unless
the log already records the maintainer's choice to run without notifications. If the log records a landing claim,
`jj git fetch` and run `queue status` and `queue history <slug>`, then go by the verb the log says you were running:

- Nothing pending (you were landing it): if the line carries your claim id, carry on with that plan from step 2 of the
  round below, after the cross-plan review that opens the round (main may have moved while the monitor was down). Read
  which of its PRs GitHub shows merged rather than trusting the log's list, since a merge may have happened after the
  log was last written. If the line no longer carries your id, the maintainer released the claim: clear it and leave the
  plan to the next round.
- `starting`: your id on the line means the claim landed; carry on as above. Otherwise it never landed: clear it.
- `blocking`, `stopping` or `recording`: if `history` shows that commit with your claim id, it landed; clear the claim
  from the log, and for `blocking` or `stopping` notify, since the crash may have come before the notification went out.
  If not and your id is still on the line, run the verb again. Otherwise the maintainer released the claim: clear it and
  leave the plan alone.

A dirty working copy found here belongs to the recorded landing (a rebase or a fix in progress): carry on with it when
the landing carries on, and reset it the way an executor's close does (Clean-main boundary) when the claim was cleared.
Only a dirty working copy with no recorded claim aborts the start, as the clean-main boundary says.

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

Before anything merges, review on purpose what has landed on main since each complete plan's stack was based, and what
is about to land together: the diffs, not the titles, read against each other. Look for interactions that produce no
textual conflict: something one change renamed or repurposed that another relies on, a contract a new caller assumes
that another change altered, a new caller of changed code, a changed invariant, lock or ordering, a SPEC.md or
SPEC_impl.md amendment that another change now contradicts. Root `AGENTS.md` "Careful rebase" lists the kinds; apply
them across the whole set, not one stack at a time. Have a fresh-context sub-agent do the same review independently and
reconcile any disagreement. Fix what is obvious as part of the landing; a conflict that needs a design decision blocks
the plan it concerns (below), and the round goes on with the others.

Then, for each `[complete]` plan in queue order:

1. Claim it. Generate a claim id (`python3 -c 'import secrets; print(secrets.token_hex(3))'`), write slug, id and
   `starting` to the monitor log, then `queue start-landing <slug> --claim <id>`. Exit 10 means it is no longer complete
   (the maintainer abandoned or released it): skip it. Exit 3 follows the rule under States.
2. Fetch, and rebase its stack onto `main@origin` as a careful rebase. Conflicts in the shared bookkeeping files listed
   under Picking are mechanical: keep both sides. Main has moved since the review above if an earlier plan just landed;
   check what that plan changed against this one again.
3. Confirm no PR in the stack touches `plans/`.
4. Run the checks the rebase calls for, per root `AGENTS.md` "Finishing work".
5. Land the stack bottom-up per `jjstack`, marking each PR ready as it lands, and record each merged PR in the monitor
   log. Before each merge, run `queue status` and confirm the line still carries your claim id; if it does not, the
   maintainer released it, so stop landing that plan and treat it as released (Starting and resuming). Bookkeeping
   commits make main move often, so a merge refused because the base branch changed means re-read and retry, not stop.
6. Write the landing notes. They become the report's `### Landing` section, which the maintainer reads after the fact,
   so follow Writing for the maintainer and public-repo hygiene, and use `####` or deeper headings if any. Say what else
   landed in the same round or since the stack was based and whether any of it interacted with this plan, which fixes
   the landing made and why, the checks run, reused and skipped (as redacted commands and recorder run ids, per root
   `AGENTS.md`), and anything in the executor's report that the landing made untrue.
7. Write `recording` to the monitor log, then
   `queue landed <slug> --claim <id> --merged <every PR number that landed> --notes <file>`, which refuses while any of
   the plan's PRs is still open and checks that each named PR merged. Clear the claim from the monitor log, and return
   to a clean `main` (`jj new main@origin`).

When a landing needs a decision only the maintainer can make (a conflict that needs a design choice, checks that fail
without an obvious fix, a PR closed without merging), push nothing that is not consistent, then write the question per
Writing for the maintainer, naming which of the plan's PRs already merged and what main now holds of the plan. Have a
sub-agent cold-read it as in Executing one plan step 10, write `blocking` to the monitor log, run
`queue block <slug> --claim <id> --question <file>`, notify, return to a clean `main`, and go on with the next plan.

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

"Monitor for complete plans" waits the way a monitoring executor does (Draining above), with the same watcher run the
same way, except for its role and what its exits mean:

- Start
  `scripts/plans-watch.sh --repo <OWNER/NAME> --baseline-from <commit> --wake-check <checkout>/scripts/plans-queue.py --wake-for lander`,
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
  `stopped` with the reason, and stop. Otherwise keep restarting the watcher with `--interval 1800` until one exits
  `changed` or `idle`, which also restores the default interval; an outage still being retried is not notified.
- `usage:` (status 2) or anything else: notify, write `stopped` with the reason, and stop.

Rewrite the Status section on every wake, at the end of every round, when stopping, and whenever you check on a long
validation run, so a landing that takes hours does not look like a dead monitor; when the maintainer says stop, write
`stopped <time>: stopped by the maintainer`. "Land the complete plans" writes `stopped <time>: one round finished` when
its round ends, since no watcher follows it. The monitor notifies only for the conditions this section names (a landing
it blocked, a landing it could not finish, the three `queue status` flags, and monitoring that stopped for any reason
other than the maintainer), each of which can stop progress until the maintainer acts. A landing that went through is
not worth a notification; it shows up in `REPORTS.md` for the next review.

## Releasing: "release plan X"

Only on the maintainer's request, for a plan whose executor or monitor died or is stuck: `queue release <slug>`. A plan
that was in flight goes back to `[pending]`, and whoever claims it next resumes it from its working log and open PRs
(Executing one plan, step 7). A plan that was landing goes back to `[complete]`, and the next landing round lands it,
starting from whichever of its PRs have not merged yet; if all of them had merged, that round only records the landing
(the `complete, no open PRs, merged ...` flag above). If the old executor or monitor is in fact still alive, its next
claim check or claim-holder verb finds the claim gone and it stops.

## Help: "plan help"

"Plan help", "help on plans", or similar means: summarize each flow in this file in one to three lines, then stop.
Change nothing. The flows: plan to implement TODO items; schedule triage outcomes through the planning system; pick a
plan to execute (and the in-order and named variants); drain the plans; drain and keep monitoring; monitor for complete
plans, and its one-round form, land the complete plans; review the plans, including show (or check for) blocked plans;
release plan X; abandoning, which happens inside review; and how notifications reach the maintainer through ntfy.

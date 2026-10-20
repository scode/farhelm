# Guard a trashed checkout's recorded location against a stale recovery attempt

Written against main at 6d9615a7 on 2026-10-20, revising the earlier reproduction plan under the same name. This is a
goal file for an unattended run, executed through the planning system in `plans/AGENTS.md`. You, the executing agent,
have none of the conversation that produced it; everything you need is here or in the repository files it names. SPEC.md
and SPEC_impl.md stay authoritative over anything this file says.

This plan has no dependency on another plan.

## The goal

When a session with a managed checkout is deleted, the supervisor moves the checkout's folder into the trash, recording
the destination first so a crash in the middle can be finished at the next startup. A review found a narrow case where
that recovery records the wrong place: if a startup is cancelled while its recovery is still running, a second
supervisor can run the same recovery, move the folder, and record where it went; the first, stale attempt can then
overwrite that record with a fresh name and fail its own rename. The record then names a folder that does not exist,
while the files sit in the trash where the trash list never shows them. No files are lost, and the trigger is rare.

The maintainer first asked for a reproduction. An earlier run of this plan tried that and was stopped after its agent
was held up by its model's safety safeguards: reproducing meant driving competing supervisors over the state-folder lock
and pausing one mid-recovery. On 2026-10-20 the maintainer chose instead to fix the harm directly, without reproducing
the race: make the write of the recorded trash location refuse to overwrite a value it did not read, so a stale attempt
fails before it renames anything, with a reasonable complexity gate (see Fallbacks).

Carry out this triage outcome, recorded under its own heading in `TRIAGE_OUTCOMES.md`. Read the entry in full first: its
Assessment says what is wrong, its Decision holds the maintainer's words (the 2026-10-20 decision supersedes the
2026-10-18 one), and its Completion criteria are the acceptance criteria.

1. `cancelled-startup-loses-archive-location.md` (`fix code`): A cancelled startup worker can overwrite the recorded
   archive location after a successor moves the checkout.

### Acceptance criteria

- Updating a checkout record's journaled trash destination succeeds only when the record still holds the destination the
  caller last read (or the state the caller expects), so an attempt working from a stale reading is refused and does not
  go on to rename anything. Every place that rewrites that destination goes through the guard.
- A refused update leaves the record and the files untouched and surfaces as the existing error for a record in the
  wrong state, which recovery and Delete already handle; the up-to-date attempt's result stands.
- Tests show it directly and sequentially: write a pending record, change its destination as a second attempt would,
  then perform the first attempt's update from its stale reading and confirm it is refused, the record still names the
  real destination, and no rename was attempted. Ordinary collision retries by a single attempt keep working. No test
  starts competing supervisors, cancels a startup, contends for the state-folder lock, or adds pause or delay hooks.
- One PR, type `fix`, with a changelog fragment (`kind: fixed`), that removes
  `review_feedback_queue/cancelled-startup-loses-archive-location.md` and its `INDEX.md` line and updates the outcome's
  Execution field. SPEC.md or SPEC_impl.md change only if they describe the journal update in a way this contradicts.

## Requirement sources

**The user's decisions:** the outcome's Decision field. On 2026-10-18 the maintainer asked for a reproduction first; on
2026-10-20, after that run was stopped, they chose this direct fix "with a reasonable complexity gate. doesn't have to
be super simple but it can't explode into thousands of lines of code". Review gate: a single fresh-context Opus 5.5
agent at high effort with the general charter, no swarm. No-workhorse mode, which `plans/.agents/planning.md` requires.

**Binding repository constraints:** `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes";
`AGENTS.md`, "Changelog fragments", "Conventional Commits" and "Tests and validation", with `.agents/validation.md`;
`.agents/test-authoring.md` for the tests.

**Planner proposals:** the outline below. A planner proposal is not a requirement: a smaller design that meets the same
acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them): the destination is rewritten on an already-pending row
by `repoint_journal` in `crates/farhelm-supervisor/src/working_copies.rs`, whose `UPDATE` is guarded only by the row id
and the `archive_pending` state; the collision branch of `rename_exclusive_into` calls it before trying the rename,
which is where a stale attempt overwrites a successor's destination. The narrowest fix is a compare-and-swap: pass the
destination the caller believes is current and add it to the `UPDATE`'s condition, so zero rows changed means someone
else moved the record on. Check the other writers of the destination (the first journal persist and recovery's own
paths) and guard them the same way where the same stale-reading overwrite is possible.

The underlying cause, a cancelled startup leaving recovery running without owning the state folder, is out of scope: the
overwrite is the only harm the finding establishes, and fixing ownership is the larger change the maintainer chose not
to pursue here.

An earlier run of the previous version of this plan (the reproduction) left a working log, described under "Files
outside the repository", and possibly an unbookmarked scratch change in its own scratch workspace. Nothing from it was
pushed. Treat that log's earlier entries as history: do not resume, reuse or run the reproduction work.

### Validation

Per `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression in what the PR changes, and report checks run, reused and skipped. At least the supervisor's
working-copy and checkout-trash tests run through `python3 scripts/record-test-run.py` with the pinned nextest and tmux
setup, plus the tests that exercise Delete of a managed checkout and startup archive recovery.

## Fallbacks

Complexity gate: at most about 200 lines of non-test code and about 500 lines in total including tests, confined to the
supervisor's checkout-record code (`working_copies.rs` and its direct callers), with no new locks, threads, background
tasks or ownership machinery. If the fix cannot meet the acceptance criteria within that gate, do not build past it:
push nothing beyond what is consistent and block with the concrete reason and the size it would take.

Examples of departures that need the scope reassessment: fixing the startup ownership itself, changing the database
schema, or touching the desktop app's startup.

Decisions to log: which destination writers were guarded and why, how a refused update is reported, and how each test
establishes its premise.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-archive-recovery-reproduction-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": one reviewable commit, stable
  bookmark and draft PR, as described under Acceptance criteria. The PR carries the fix, its tests and changelog
  fragment, the removal of the feedback file and its `review_feedback_queue/INDEX.md` entry, and the
  `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and bookmark in that Execution field before creating
  the PR, then add the PR's URL (with `/changes` appended) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/archive-recovery-reproduction/<nn>-<short-name>` (for example
  `plan/archive-recovery-reproduction/01-guard-archive-destination`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `fix` for a change users of Farhelm can observe, with a changelog fragment in the same commit; `test`
  for a change only to tests or their fixtures; `docs` for documentation and specification text only; `chore` for
  maintainer tooling and queue bookkeeping. Only a fragment-requiring type adds a fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TRIAGE_OUTCOMES.md`, `review_feedback_queue/`, `TODO.md` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry, including any complexity gate (ask the reviewer to flag anything beyond the gate).

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above. If current code or specs invalidate a recorded triage decision (for example,
the finding turns out already fixed or wrong), that is a block: `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes", returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every outcome that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

The review gate's acceptance criteria include the complexity gate under Fallbacks above: ask the reviewer to flag
anything beyond it, and any test that races supervisors, cancels a startup or adds pause hooks.

## Done criterion

The plan is complete when one draft PR carries the guarded destination update, its tests and changelog fragment, the
removal of the feedback file and its index line, and the outcome's Execution field (with the change ID, bookmark and
PR), and that PR passed its review gate. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver
its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/`
yourself.

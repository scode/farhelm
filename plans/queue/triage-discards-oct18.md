# Queue bookkeeping: the five discards from the 2026-10-18 triage

Written against main at 319b286a on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The maintainer discarded five findings under the 2026-10-11 rule (rare trigger, no shown damage). A discard means only
"not worth the maintainer's time at this time", not that the finding is wrong. Executing them removes each feedback file
and its index line and records the execution in the ledger, with no spec or code change. The maintainer chose one PR for
all five, rather than the procedure's one PR per outcome.

Carry out these 5 triage outcomes, each recorded under its own heading in `TRIAGE_OUTCOMES.md`, in this order. Read
every entry in full first: its Assessment says what is wrong and which rule settles it, its Decision holds the
maintainer's words, the planned fix and any complexity gate, and its Completion criteria are the acceptance criteria for
that outcome. Ledger order.

1. `orphan-git-outlives-cache-lock.md` (`discard`): Interrupted checkout preparation can leave Git working after its
   repository-cache lock is released.
2. `restart-abort-admits-old-child-report.md` (`discard`): An aborted Restart can admit an old child conversation report
   into the restored run's Resume state.
3. `capture-marker-collision-misframes-replay.md` (`discard`): Terminal text can imitate capture delimiters, truncating
   replay or entering diagnostics.
4. `initial-launch-retains-uncanonicalized-path.md` (`discard`): Initial launch retains a path spelling that can change
   meaning after validation.
5. `tunnel-never-pins-learned-identity.md` (`discard`): A tunnel opened before identity is learned can keep following a
   host entry without binding the subsequently learned installation.

### Acceptance criteria

- One PR (type `chore`, no changelog fragment) removes all five feedback files and their
  `review_feedback_queue/INDEX.md` lines and sets each outcome's Execution field to complete, naming the change ID,
  bookmark and PR. No spec, code or TODO change.
- `review_feedback_queue/INDEX.md` still matches the files in that directory.

## Requirement sources

**The user's decisions (2026-10-18):** each outcome's Decision field records the maintainer's own words and the agreed
scope. Scheduling the 25 outcomes triaged that day as twelve plans, and which outcomes share a plan, was agreed with the
maintainer. Review gate: a single fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
No-workhorse mode, which `plans/.agents/planning.md` requires.

**Plan-specific answer (2026-10-18):** one PR for all five discards (the maintainer's choice over one PR each).

**Binding repository constraints:** the rule each entry's Assessment cites as settling it;
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"; `AGENTS.md`, "Changelog fragments",
"Conventional Commits" and "Tests and validation", with `.agents/validation.md`; `.agents/test-authoring.md` for any
test change.

**Planner proposals:** each entry's planned fix, the PR order above, and the outline below. A planner proposal is not a
requirement: a smaller design that meets the same completion criteria is fine, logged as a DECISION.

## Outline

One PR. Before removing each file, confirm its ledger entry still says `discard` with Execution `planned in` this plan;
anything else is a block, not a removal.

### Validation

Per `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression in what each PR changes, and report checks run, reused and skipped. `dprint check` on the changed
Markdown. No runtime tests: nothing but queue bookkeeping changes.

## Fallbacks

No complexity gate. A discard whose ledger entry has changed since planning is returned to the maintainer as a block
rather than removed.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-triage-discards-oct18-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": the maintainer chose one
  reviewable commit, stable bookmark and draft PR for all five outcomes, in place of the procedure's one PR per outcome.
  Each PR carries its outcome's agreed change, the removal of its feedback file and `review_feedback_queue/INDEX.md`
  entry, and its `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and bookmark in that Execution field
  before creating the PR, then add the PR's URL (with `/changes` appended) to the same change and push it again; no
  separate bookkeeping PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/triage-discards-oct18/<nn>-<short-name>` (for example `plan/triage-discards-oct18/01-discards`).
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
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
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

## Done criterion

The plan is complete when the one draft PR exists, meets the acceptance criteria above and passed its review gate, and
all five Execution fields name its change ID, bookmark and PR. Open, not merged. If a `## Decisions` section exists, its
latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps
11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.

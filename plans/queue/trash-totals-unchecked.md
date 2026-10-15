# Trash totals say when unchecked entries make them partial

Written against main at 136b62d1 on 2026-10-15. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out this triage outcome, recorded under its heading in `TRIAGE_OUTCOMES.md`. Read the entry in full first: its
Assessment says what is wrong and which rule settles it, its Decision holds the planned fix and the complexity gate, and
its Completion criteria are the acceptance criteria for that outcome.

1. `trash-host-totals-exclude-unchecked-archives.md` (`fix code`, complexity gated): A host with unreadable archived
   work can display “0 checkouts · 0 B” instead of an unknown or partial total.

### The complexity gates

Each outcome's gate is binding. The maintainer delegated these decisions on the condition that the fixes stay small, and
each ledger entry says the item returns to the maintainer if its fix cannot stay within its gate. Whether a fix meets
its gate is yours to judge and log. If one does not, do not build it: build and push the plan's other outcomes, then
block (per `plans/AGENTS.md`, Executing one plan, step 10) with a question that describes the larger fix that would be
needed and asks whether to build it or discard the outcome. The entry's planned fix is the pre-triage reviewer's
proposal; a different fix that stays within the same gate and meets the same completion criteria is fine, logged as a
DECISION.

### Acceptance criteria

- Each outcome's Completion criteria in its `TRIAGE_OUTCOMES.md` entry, including its regression test.
- Each outcome's PR removes its feedback file and `review_feedback_queue/INDEX.md` entry and updates its Execution
  field.
- A `fix` PR carries a changelog fragment (`kind: fixed`) per root `AGENTS.md`; test-only, docs-only and tooling PRs do
  not.

## Requirement sources

**The user's decisions (2026-10-15):** the delegation, in the maintainer's words: "for everything that's left in any
bucket: please assess which ones are CLEARLY settled by the spec, don't need my judgement, and can be fixed without
significant complexity or scope creep. triage those as 'fix code' with explicit complexity gates." A fresh-context
pre-triage review classed each outcome here as settled by the specs or repository rules, needing no product judgement,
and small; the triaging session checked those calls. Grouping the fifty delegated outcomes into seventeen plans was
agreed with the maintainer. Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** the rule each entry's Assessment cites as settling it; root `AGENTS.md` "Execute
triage outcomes", its changelog rule and "Finishing work"; `.agents/test-authoring.md` for any test change.

**Planner proposals:** each entry's planned fix, and the PR order above.

## Outline

One PR per outcome, in the order listed.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set of checks that would expose a regression in what each PR
changes, and report checks run, reused and skipped. Typically: `cargo fmt --all -- --check` and
`cargo clippy --all-targets -- -D warnings` when Rust changes, the touched crate's unit tests through the recorder, the
JS unit tests (`crates/farhelm-ui/js-tests`) or browser specs the outcome's completion criteria name (browser specs on
Chromium and WebKit through the recorder after the documented builds, plus the test-sleep checker), and
`python3 releasing/check-changelog.py format` for a `fix`.

## Fallbacks

The complexity gates above are the agreed fallback for every outcome.

Examples of departures that need the scope reassessment: anything outside the outcome's complexity gate.

Decisions to log: for each outcome, whether its gate was met and why, and any fix that differs from the entry's planned
fix.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-trash-totals-unchecked-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md`
(Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
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

- Follow root `AGENTS.md` "Execute triage outcomes": one reviewable commit, stable bookmark and draft PR per outcome, in
  a single linear stack in the order given under The goal above. Each PR carries its outcome's agreed change, the
  removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its `TRIAGE_OUTCOMES.md` Execution
  update. Record the jj change ID and bookmark in that Execution field before creating the PR, then add the PR's URL
  (with `/changes` appended, per the user's PR-link rule) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/trash-totals-unchecked/<nn>-<short-name>` (for example `plan/trash-totals-unchecked/01-partial-totals`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `fix` for a change users of Farhelm can observe, with a changelog fragment (`kind: fixed`) in the same
  commit; `test` for a change only to tests or their fixtures; `docs` for documentation only; `chore` for maintainer
  tooling. Only `fix` (or another fragment-requiring type) adds a fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
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
  `TRIAGE_OUTCOMES.md` entry, including its complexity gate (ask the reviewer to flag anything beyond the gate).

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, whether each outcome met its complexity gate,
and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above. If current code or specs invalidate a recorded triage decision (for example, the finding turns out
already fixed or wrong), that is a block: root `AGENTS.md` (Execute triage outcomes) returns such an item to the user
rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user, after pushing every outcome that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless an outcome
failed its complexity gate (then the plan blocks as described above), that together meets the acceptance criteria above,
each PR passed its review gate, and each built outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID,
bookmark and PR. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script,
write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

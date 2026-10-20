# Helm keeps its state-directory lock until its runtime's work has finished

Written against main at be722d41 on 2026-10-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

When the helm stops serving abnormally (a panic during a host install or update, or the desktop's embedded shutdown), it
releases its state-directory lock before the tokio runtime that runs its blocking work has shut down. A successor helm
can then claim the same state directory while the old helm's install and update work is still writing to the shared
release cache. Make the lock outlive every task started on the helm's runtime.

Carry out this triage outcome, recorded under its own heading in `TRIAGE_OUTCOMES.md`. Read the entry in full first: its
Assessment and the reproduction evidence say what is wrong, and the latest Outcome, Decision and Completion criteria
near the end of the entry (dated 2026-10-19) supersede the earlier `other` outcome and are the acceptance criteria.

1. `payload-workers-outlive-state-lock.md` (`fix code`): Payload work can outlive the helm's state-directory lock.
   Size-gated at about 300 lines.

### The size gate

The gate is binding. In the maintainer's words: "the gate shouldn't be in time (these time estimates are useless
anyway), just have it block if it's more than.... 300 lines". It counts the diff's code, tests and docs, not
`TRIAGE_OUTCOMES.md`, the queue removal or the changelog fragment. If the change cannot be done within it, block per
`plans/.agents/executing.md`, "Executing one plan", step 10, with a question that describes the larger fix, its size,
and why the smaller one does not work, and asks whether to build it or discard the outcome. Measure and log the size.

### Acceptance criteria

- The outcome's Completion criteria in its `TRIAGE_OUTCOMES.md` entry (the 2026-10-19 ones), including the regression
  test.
- The PR removes the feedback file and its `review_feedback_queue/INDEX.md` entry and updates the Execution field.
- A changelog fragment per `AGENTS.md`, "Changelog fragments". The visible effect is small (a second helm started during
  the old one's drain is refused instead of sharing the state directory), so `kind: none` with that reason is
  acceptable; write `kind: fixed` if you find a visible effect worth an entry, and log which.

## Requirement sources

**The user's decisions (2026-10-19):** the Decision fields of the entry, including the size gate quoted above. When
planning, the user also accepted that `farhelm helm token rotate` (and anything else needing token control) may fail
while the old helm drains: the token-control socket goes away when serving ends, while the lock is now held a little
longer. The planning question described this as a busy error for `token show` and `token rotate`; the planning review
then found that `token show` falls back to reading the token from the database and keeps working, and only `rotate`
fails, after its deadline (`token_control.rs` around 215–223 and 282–315). That is narrower than what the user accepted.
Review gate: a single fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm. No-workhorse
mode, which `plans/.agents/planning.md` requires.

**Binding repository constraints:** `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes";
`AGENTS.md`, "Changelog fragments", "Conventional Commits", "Tests and validation" and "The live install is off-limits"
(the test runs its own helm from a temporary state directory, never the maintainer's), with `.agents/validation.md`;
`.agents/test-authoring.md` for the test. SPEC_impl.md, "Who owns an accepted action".

**Planner proposals:** the outline below. A planner proposal is not a requirement: a smaller design that meets the same
completion criteria is fine, logged as a DECISION.

## Outline

One PR. Facts from the planning session's reading of main (verify them):

- The lock is taken in `run_with_ready` (`crates/farhelm-helm/src/lib.rs` around 2025) and handed to the control guard
  (`token_control.rs` around 79–93 and 203–208), which drops it when `run_with_ready` returns or unwinds. In practice
  serving ends only by a panic or the desktop's embedded shutdown signal (`axum::serve` and `watch_revision` never
  return). Tokio's runtime drop waits for started blocking tasks, but it runs after the lock is gone.
- Production callers: `farhelm helm run` (`crates/farhelm/src/main.rs` around 1309) and the desktop helm thread
  (`crates/farhelm-ui/src/desktop.rs` around 503). One in-crate test calls `crate::run` (`lib.rs` around 2562).
- Proposed design: turn the public helm entry points into synchronous functions that build the runtime themselves and
  declare the lock slot before it, so locals drop in reverse order on return and on unwind: the runtime first (waiting
  for its work), the lock second. `run_with_ready` stores the lock in that slot instead of in the control guard. Replace
  the async entry points rather than adding sync ones beside them, so no public API keeps releasing the lock early;
  point the in-crate test at `run_with_ready` or a private async wrapper. No `catch_unwind`, `Arc` or `Mutex` should be
  needed. Do not use `shutdown_timeout`: it would reopen the gap.
- Ordinary Quit exits the process without destructors and stays as it is. The desktop helm thread already drops its
  runtime before returning, and `DesktopBootstrap::drop` already joins it, so exit timing does not change; only the
  lock's release moves later.
- Test: run the real helm in-crate on port 0 with a temporary state directory (precedent around `lib.rs` 2556–2570,
  `base_args()`). One narrow `cfg(test)` call site after the ownership claim spawns a blocking task that parks on a
  channel; trigger shutdown; while parked, a non-waiting `acquire_ownership` (`token_control.rs` around 512, not the
  5-second waiting claim) is refused; release the task, join, and the probe succeeds. The test fails before the fix. No
  general hook framework. The reproduction patch in `plans/reports/payload-lock-reproduction.report.md` has reusable
  readiness and release plumbing, but it ran the helm on the test's own runtime, so its structure does not carry over.
- SPEC_impl.md, "Who owns an accepted action": one sentence that the helm's state-directory claim outlives all work
  started on its runtime.

### Validation

Per `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the `farhelm-helm` tests through the recorder with a narrow selection
(the new test and the helm's existing ownership and token-control tests), the test-sleep checker,
`python3 releasing/check-changelog.py format`.

## Fallbacks

The size gate above is the agreed fallback.

Examples of departures that need the scope reassessment: making the payload cache safe for concurrent writers, a shared
lock copy carried into every payload task, a shutdown timeout, or a general test-hook framework.

Decisions to log: how the lock slot is wired, how the test parks work on the helm's runtime, the changelog fragment's
kind, and the measured diff size against the gate.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-helm-lock-outlives-work-log.md` in the parent directory of the checkout you run in, derived as that
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
  bookmark and draft PR per outcome, in a single linear stack in the order given under The goal above. Each PR carries
  its outcome's agreed change, the removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its
  `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and bookmark in that Execution field before creating
  the PR, then add the PR's URL (with `/changes` appended) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/helm-lock-outlives-work/<nn>-<short-name>` (for example
  `plan/helm-lock-outlives-work/01-lock-outlives-runtime`).
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
  `TRIAGE_OUTCOMES.md` entry, including its size gate (ask the reviewer to flag anything beyond the gate).

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
alternatives considered: in particular those named under Fallbacks above, whether each gated outcome met its gate (with
the measured diff size) and why, and every review finding you decided not to follow. The user will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal (an outcome whose
size gate trips makes the plan block instead, after its other outcomes are pushed), that together meets the acceptance
criteria above, each PR passed its review gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change
ID, bookmark and PR. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

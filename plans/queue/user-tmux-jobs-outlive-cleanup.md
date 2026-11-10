# Jobs from a user-run tmux may outlive session cleanup

written against main at 841e4715 on 2026-11-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute two triage outcomes recorded on 2026-11-10 as one decision, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Their `TRIAGE_OUTCOMES.md` entries are
the binding statement of what to do; read both in full from `main@origin`, together with their feedback files under
`review_feedback_queue/`:

1. `scoped-cleanup-misses-late-tmux.md` (`fix spec+code`)
2. `scoped-cleanup-forgets-ended-tmux.md` (`fix spec+code`)

The ledger says one change executes both, so this plan builds one PR for the pair (an exception to execute-outcomes'
one-PR-per-outcome rule that the decision itself makes). Its Execution fields both name that PR. Where the done
criterion below says "one per triage outcome", read it as this one PR covering both outcomes.

### What the PR does

The tmux here is one the user or an agent starts inside a session, not Farhelm's own tmux-based terminals. On Linux
hosts where Farhelm puts each agent launch and terminal tab in a systemd scope, cleanup (Stop, Restart, Replace, Delete,
tab close) trusts the scope instead of sweeping every process on the host. Stock tmux moves each pane's program into its
own `tmux-spawn-*.scope`, so a pane job that ignores hangup survives the scope's end. Since 2026-11-09 the reaper checks
the scopes for a running tmux server before the first kill and keeps the machine-wide sweep when it finds one
(`crates/farhelm-supervisor/src/service/sweep.rs`, `reap_process_tree_with_sweep`, the pre-kill loop calling
`needs_tmux_sweep`; `crates/farhelm-supervisor/src/scope.rs`, `needs_tmux_sweep` and `member_needs_tmux_sweep`). The two
findings are the cases that check misses (a server started after the check, a server that had already ended).

The maintainer decided, in their words: "if the issue is just that user started tmux sessions don't get killed, that's
fine. this is essentially the intent the user has when using tmux, and we can call it out in the spec." Asked whether to
keep the pre-kill check as best effort or remove it, they picked "Say these jobs survive, and remove the check". So the
PR:

- removes the pre-kill tmux check and everything that exists only for it (the tmux member classification and its unit
  tests), so scoped cleanup never runs the machine-wide sweep because of tmux;
- keeps the other rule that pre-kill inspection enforces today. SPEC_impl.md's process-tree section says "scope-only
  cleanup applies only while every named scope is still live": a scope that is already gone (the agent exited before
  Stop, say) keeps the sweep. Today only the tmux inspection notices a gone scope, because it reads no members and
  answers "keep the sweep"; `kill_scope` reports an already-gone scope as success. Deleting the loop outright would
  silently drop that rule, so the PR keeps a liveness check: the planner's proposal is to have `kill_scope` (or the
  existence check it already makes) report "already gone" and keep the sweep on that, rather than keeping a membership
  read. A test pins that a gone scope still sweeps;
- removes or inverts the end-to-end regression for it (`crates/farhelm/tests/e2e/session_lifecycle.rs`, the test that
  starts `/usr/bin/tmux` with a hangup-ignoring pane job and expects Stop to end it); inverted, it would pin the new
  rule that the job survives a scoped Stop, which is worth keeping only if it stays cheap and deterministic;
- rewrites the spec text that promised the opposite: SPEC.md "Lifecycle operations" (the "Confirmed 2026-11-09" sentence
  that "before killing any scope it would trust, cleanup checks those scopes for a running tmux server") and "Ownership
  during cleanup and provisioning" (the sentences about a tmux server inside a scope keeping the portable sweep and
  "Ordinary tmux pane migration on a scoped launch does not weaken this promise"), the "Confirmed 2026-10-22" paragraph
  there about a per-user shared program such as the user's own tmux server ending "along with everything later run in it
  or through it", which needs narrowing for migrated pane programs, and SPEC_impl.md's matching "Confirmed 2026-11-09"
  paragraph in the process-tree section. The new text says that programs a user-run tmux places outside the session's
  process groups are not ended by Stop, Restart, Replace, Delete or tab close on scoped hosts, as the intent of running
  them under tmux, while the tmux server itself, being in the scope, still ends. The module docs in `scope.rs` and
  `sweep.rs` that describe the check are updated too;
- leaves unscoped hosts' sweep unchanged;
- edits the existing changelog fragment `releasing/changelog.d/scoped-cleanup.md` instead of adding one: scoped cleanup
  has not shipped in a stable release yet, so root `AGENTS.md`, "Changelog fragments", says an amendment edits that
  fragment. It currently promises that programs in a tmux server's panes still end; it now says jobs you start under
  your own tmux inside a session can keep running after Stop, Restart or Delete, as they would outside Farhelm. (The
  ledger's "a changelog fragment states the new behavior (`changed`)" is met this way.)

Pick the Conventional Commit type from the user-visible effect (root `AGENTS.md`, "Conventional Commits") and log the
choice.

### Acceptance criteria

- One PR meeting both ledger entries' completion criteria, recording its execution in both Execution fields, removing
  both feedback files and their `INDEX.md` lines, and moving both entries to `TRIAGE_OUTCOMES_ARCHIVE.md`.
- No code path in scoped cleanup inspects scopes for tmux; the other reasons to keep the sweep (incomplete containment,
  a scope already gone before cleanup, an unconfirmed scope, a surviving root pid) still hold, each covered by a test.
- SPEC.md and SPEC_impl.md no longer promise that pane jobs a user-run tmux migrated end with the session, and say so
  plainly as accepted behavior.

## Decisions already made

- **The outcome:** the maintainer's words quoted above, and their follow-up pick "Say these jobs survive, and remove the
  check" (2026-11-10 triage page), which reverses the 2026-11-09 decision that added the check.
- **Size gate:** about 100 lines of added code, tests and docs; removed lines do not count, nor do the ledger, the queue
  removal or the changelog fragment. Over it, block and bring it back rather than expanding.
- **Grouping:** one plan per outcome, with this pair as one plan, approved on the 2026-11-10 planning page.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the two `TRIAGE_OUTCOMES.md` entries named above, and the maintainer's words quoted there.

**Binding repository constraints:** the rest of SPEC.md "Lifecycle operations" and "Ownership during cleanup and
provisioning" (scoped cleanup trusting the scope, its other sweep conditions, the deliberate-escape acceptance), the
SPEC_impl.md process-tree section (in particular "scope-only cleanup applies only while every named scope is still
live"), root `AGENTS.md`, "Tests and validation", "Changelog fragments", "Conventional Commits";
`.agents/test-authoring.md`; `review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): delete the tmux classification in
`scope.rs` and the pre-kill loop in `reap_process_tree_with_sweep`, replacing the loop's gone-scope effect with an
"already gone" result from `kill_scope`; keep the e2e test only if inverting it is cheap and stable, otherwise delete it
and say so in the PR.

## Outline

1. `plan/user-tmux-jobs-outlive-cleanup/01-drop-tmux-sweep-check`: the code removal, its tests, the spec rewrites, the
   fragment, and the ledger and queue bookkeeping for both items.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-supervisor`, the supervisor's sweep and scope unit tests through `python3 scripts/record-test-run.py`, the
scoped-cleanup e2e tests in `crates/farhelm/tests/e2e/session_lifecycle.rs` that touch the changed path (these need a
systemd user manager; read their SKIPPED output per root `AGENTS.md`), `python -B scripts/check-test-sleeps.py` if tests
change, `dprint check` on the spec files and the fragment, and `python3 releasing/check-changelog.py format`.

## Fallbacks

If the scoped-cleanup code has moved since this was written, follow the current code and correct this file's names to
the truth, logged as a DECISION. If something else on main now relies on the tmux inspection (another sweep reason, a
diagnostic), keep what it needs and remove only the tmux-driven sweep, logged as a DECISION.

Examples of departures that need the scope reassessment: any new detection of tmux pane scopes, freezing scopes, or a
setting to opt back into the sweep.

Decisions to log: what was removed versus kept, the e2e test's fate, the commit type, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-user-tmux-jobs-outlive-cleanup-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/user-tmux-jobs-outlive-cleanup/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per triage outcome in the order the outline gives,
each meeting that outcome's completion criteria in `TRIAGE_OUTCOMES.md` (as refined by Decisions already made above),
each having passed its review gate, and each recording its execution in the outcome's Execution field (change id,
bookmark and PR URL), removing its feedback file and `review_feedback_queue/INDEX.md` line, and moving its entry to
`TRIAGE_OUTCOMES_ARCHIVE.md`, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

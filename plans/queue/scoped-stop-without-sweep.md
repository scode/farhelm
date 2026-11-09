# Stop, tab close and Delete trust the systemd scope and skip the machine-wide sweep

Written against main at e7b47815 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's goal is to change them, as stated below.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Make the process sweep cheaper where scopes exist.** The stop, tab-close and Delete sweep finds escaped processes by
> reading the environment of every process on the machine (`/proc/<pid>/environ`), which costs one read per process on
> the host (about 900 on a dev box on 2026-10-28, against 5 in one session's scope). Where a launch or tab has its own
> scope, selecting by the scope's `cgroup.procs` covers the same processes for a fraction of the cost. SPEC_impl.md
> calls the marker sweep the backstop that always runs, including where a scope exists, because a scope can be left by a
> process that runs `systemd-run --user --scope` itself; changing that is a SPEC decision, so weigh what the backstop
> still catches against its cost before changing it. Hosts without a usable user manager keep the full sweep.

The maintainer made the SPEC decision while planning, in their words: "trust the group. when systemd is used, just do
systemd. SPEC.md that if folks run things that intentionally moves into a separate group farhelm won't kill it." And,
asked whether the process-tree walk (which also reads every process on the machine) goes too: yes, the group kill plus
ending the terminal itself, nothing machine-wide. (While planning, a review found that ending the terminal needs no new
step: the scope wrapper execs in place, so the terminal's process is the agent, and each operation already does what it
should to its terminal.)

What is true today, from a read of main at e7b47815 (line numbers approximate; correct them to the truth as you go):

- **The mechanism:** `crates/farhelm-supervisor/src/service/sweep.rs` (module docs at the top; `reap_process_tree`
  around 1359; `kill_process_tree` around 1096; `kill_scope` around 1653 and the scope confirmation around 1790-1814).
  Process-table reads go through `crates/farhelm-supervisor/src/procs.rs` (`/proc` on Linux, `sysctl` on macOS). Stop
  today: kill through the launch's or tab's systemd scope (SIGTERM, grace, SIGKILL, confirm the unit was collected),
  then ALWAYS the portable sweep: the pane's descendant tree (PPID closure over the whole process table) united with
  every same-user process whose environment carries the session's markers (`FARHELM_SESSION_ID` plus `FARHELM_AGENT_ID`
  or `FARHELM_TAB_ID`), then SIGTERM, grace, SIGSTOP quiesce, re-enumerate, SIGKILL, confirm.
- **The entry point:** every Stop, Restart, Delete, tab close, automatic tab cleanup, host restart and failed-create
  rollback goes through `reap_process_tree` (about 15 call sites). Some callers refuse on a cleanup failure and some
  only warn (the failed-create rollback and the host relaunch unwind in `service/host_restart.rs` around 1207 and 1244),
  and Restart passes explicit roots for launches that earlier host starts left running (`settle_outstanding_launches`,
  around 971) while the scope units it names cover only the current generation.
- **Tests that pin today's rule:** `the_backstop_sweep_runs_after_the_scope_kill_never_before` in `sweep.rs` around
  2636; and two runtime tests that assert a marked process outside the scope dies on a scoped Stop:
  `a_scope_launched_stop_kills_through_the_cgroup_and_still_runs_the_sweep` in
  `crates/farhelm/tests/e2e/session_lifecycle.rs` around 5769 (its `MarkedDecoy`), and
  `a_session_marked_process_with_no_kind_marker_is_still_reaped_by_stop` in `crates/farhelm/tests/e2e/marker_model.rs`
  around 100, which runs scoped on a host with a user manager.
- **The specs:** SPEC_impl.md around 4970-5100 (process-tree ownership; "The sweep ALWAYS runs afterwards as the
  backstop" around 5034; "What the cgroup does and does not promise"; the residuals for hosts without a usable user
  manager). SPEC.md's Lifecycle operations (around 1777-1807: "Process-tree ownership is session-wide", the exclusion of
  "hostile same-account processes deliberately escaping cleanup", the scope-must-be-confirmed rule) and around 4581
  state the user-facing promises.
- **What scoping covers:** each agent launch gets a generation-named scope where a functional user manager exists; the
  scope wrapper execs in place, so the pane's process is the agent itself, and the login shell and waypoint exec onward
  rather than surviving beside it. Each terminal tab's scope wraps the tab shell. A launch made while the manager did
  not answer runs unscoped. Containerized sessions revoke or remove their container around the reap (`sweep.rs` around
  1974, `service/teardown.rs` around 566).

### Acceptance criteria

- **One rule, inside `reap_process_tree`:** skip the machine-wide sweep (`kill_process_tree`: the marker scan and the
  PPID walk over every process) when (a) this reap named at least one scope unit and the user manager was usable, (b)
  every named unit's scope kill was confirmed collected, and (c) no validated explicit root passed in is still alive
  afterwards (checking only those roots, never the process table). Otherwise run today's sweep unchanged. That makes
  failure handling unchanged for refusing and warning callers alike, keeps reaping roots the current scope does not hold
  (Restart's earlier-generation roots, Delete's pane roots), and sends any ambiguous scope state to the full sweep
  without a separate rule. If you find a reason this predicate is wrong for some caller, that is a scope reassessment,
  not a per-caller carve-out.
- **Terminal handling is unchanged:** each operation keeps whatever it already does to its pane, tabs or terminal host
  (Stop and Restart keep the pane; Delete ends hosts after the reap). No new "end the terminal" step.
- **Everything without a scope keeps the full sweep:** macOS, hosts without a usable systemd user manager, launches made
  while the manager did not answer, and legacy rows and windows the existing re-probe rules treat as unscoped.
- **Containerized sessions:** container teardown is unchanged. If skipping the sweep would change what reaches a
  container's processes, fall back to the full sweep for containerized launches and log the DECISION; do not redesign
  container teardown.
- **SPEC.md**, in Lifecycle operations, says that for a launch or tab Farhelm runs in a systemd scope it ends that
  scope, and does not stop (1) a program that moves itself into a separate group (for example by running
  `systemd-run --user --scope`) or (2) a process that carries the session's environment marker without ever having been
  in the scope (something the user started by hand with the marker set, or a daemon left from before markers existed).
  **SPEC_impl.md**'s process-tree section replaces "the sweep ALWAYS runs afterwards as the backstop" with the predicate
  above, keeps the residuals accurate, and records that this was the maintainer's decision on 2026-11-08 trading the
  backstop for cost. Update the `sweep.rs` module docs and doc comments that state the old always-runs rule.
- **Tests:** rewrite the unit test pinning the old order to pin the new rule (scoped and confirmed: no process-table
  reads; scope failure or a surviving root: full sweep; unscoped: full sweep), using the supervisor's existing seams
  where they exist. In the runtime tests above, keep the cloaked-daemon half (a process inside the scope that scrubbed
  its markers still dies); flip the decoy half to "a marked process outside the scope survives a scoped Stop", or move
  it to an unscoped-only test; run the legacy no-kind-marker test only where the launch is unscoped. Other runtime
  coverage of scoped stops keeps passing.
- Typed `perf`, with a changelog fragment that states the caveat plainly: on Linux hosts with systemd, a program that
  moves itself out of its session's group, or one that only carries the session's marker, is no longer stopped.
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **Trust the group.** Where a launch or tab has a scope, the scope is the mechanism; the always-run marker sweep goes.
  The alternative, keeping the full sweep as the backstop, was rejected for its cost (about 900 process reads per stop
  on a dev box, against about 5 processes in the scope).
- **The process-tree walk goes too** where a scope exists: the group kill plus ending the terminal itself, nothing
  machine-wide. The alternative was dropping only the marker sweep.
- **SPEC.md states the residual**: a program that intentionally moves itself into a separate group is not killed.
- **No measurement step is required**; the decision was made on the counts above. Measure only if it helps you show the
  change works.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold**, at the maintainer's request, so it runs only once they unhold it.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's lifecycle promises and SPEC_impl.md's process-tree section (changed by
this plan only as stated); `crates/farhelm-supervisor/src/agent_kind/mod.rs` module docs if anything here turns out to
be per-harness (it should not); root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments",
"TODO.md and FLAKES.md", "Sharing the machine with other agents" (test scopes carry a UUID; never touch units you did
not start); `.agents/test-authoring.md`; `.agents/narrow-tests.md` when a test misbehaves.

**Planner proposals:** the single predicate in `reap_process_tree` (so Restart's agent reap, automatic tab cleanup, host
restart and create rollback get the same rule, which is simpler than carving them out; the maintainer named Stop, tab
close and Delete); the surviving-root check (c), which keeps reaping roots the current scope does not hold; the
containerized fallback; naming the marker-only residual in SPEC.md.

## Outline

One PR, `plan/scoped-stop-without-sweep/01-scope-only`, typed `perf`: the predicate, the SPEC.md and SPEC_impl.md
changes, doc comments, the rewritten tests, the changelog fragment, and the TODO.md removal.

### Validation

The supervisor's unit tests for the sweep and scope paths, plus the runtime tests that exercise scoped Stop, tab close
and Delete on a real systemd user manager (find them per `.agents/validation.md`; read the retained output, since a
runtime `SKIPPED` early return is a pass in JUnit but proves nothing), run through `scripts/record-test-run.py`. Run a
macOS-relevant check only if the shared code path changes there (it should not). `cargo fmt --check`, clippy, and
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` if tests change.

## Fallbacks

Examples of departures that need the scope reassessment: building the delegation boundary (a supervisor-owned parent
slice), reading `cgroup.procs` to re-implement the sweep per scope, changing the probe or re-probe rules, or changing
macOS behavior.

Decisions to log: how the predicate's three conditions are evaluated, whether containerized launches needed the
fallback, the exact SPEC wording, how each runtime test was changed, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-scoped-stop-without-sweep-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/scoped-stop-without-sweep/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
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

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-11-09: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-11-09 (claim 3e9ef1).
>
> ### Landing review: scoped cleanup misses a session-started tmux server's panes
>
> The landing monitor blocked this plan before merging anything; none of its PRs (#3237) has merged, and main holds none
> of it.
>
> This plan lets Stop, closing a tab, Restart and Delete trust a launch's or tab's systemd scope and skip the
> machine-wide scan for processes carrying the session's marker. The landing review found that this breaks a rule the
> maintainer confirmed on 2026-10-22 and 2026-10-23: a per-user tmux server that an agent or a terminal tab starts first
> ends with that agent or tab, along with everything later run in it. Ubuntu's stock tmux (3.4, linked with systemd)
> moves every pane's processes into a separate `tmux-spawn-*.scope` unit, outside Farhelm's scope. Before this plan the
> marker scan still found them; with it, killing Farhelm's scope ends the tmux server, and anything in its panes that
> ignores the hangup (a detached dev server, work started in that tmux over SSH) survives Stop, Restart, tab close and
> Delete. The user does nothing unusual for this to happen, so it is not the deliberate escape the plan's decision
> accepted.
>
> The review also found that Delete can skip the scan while an unscoped tab's processes may remain: a session with no
> terminal skips tab discovery, and an old tmux session whose server died lists no tabs, so only scoped tabs are found
> through the systemd manager. That contradicts the plan's own SPEC_impl.md text ("unavailable selection keeps the sweep
> required"). About eight code comments still describe the always-run sweep (`core.rs` and `teardown.rs` mostly).
>
> Options:
>
> - Keep the full sweep whenever a tmux server is running inside a scope the cleanup trusts, checked before the scope is
>   killed, and fix the Delete gap and the stale comments. Sessions that start no tmux server keep the cheaper cleanup.
> - Accept the tmux case as a documented limit of scoped cleanup in SPEC.md and the changelog entry.
>
> Recommendation: the first option, which keeps the confirmed tmux promise at little cost.

The maintainer's answer:

The maintainer chose the first option on 2026-11-09, in conversation with the landing monitor.

- Before trusting a scope, Stop, tab close, Restart and Delete check whether a tmux server process is running inside any
  scope they would trust; if one is, they keep the full portable sweep. Sessions and tabs that start no tmux server keep
  the scope-only cleanup. Add an e2e test with a real stock tmux started inside the session's scope, whose pane runs a
  process that ignores SIGHUP, showing Stop ends it.
- Delete keeps the full sweep whenever its tab discovery is incomplete: the session has no terminal, or an old tmux
  session's tmux listing cannot show its tabs (for example its server is gone). Add a test for it.
- Update the comments that still describe the always-run sweep (the landing review named `core.rs` near the "both
  asked", "falls through to the sweep" and related comments, `teardown.rs` near line 336, `host_restart.rs` near line
  1206, test messages in `sweep.rs`, and the docstring in `agent_listing_real_stack.rs`).
- Make SPEC.md and SPEC_impl.md say exactly this, and keep the changelog entry consistent with it.

The landing review's full findings, including two optional notes (a crashed later-generation host start on Restart, and
login shells such as fish that never get the saving), are for the executor's judgment.

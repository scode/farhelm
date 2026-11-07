# No git cleanup while a launch waits; the repository cache consolidates in the background after every launch

Written against main at 66e77cf4 on 2026-11-07. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Don't let git auto-gc run while a session is spawning.** On 2026-11-02, git auto-triggered gc on the farhelm repo
> itself when spawning a session — the user was actively waiting, which is exactly when auto-triggering is supposed to
> be deferred. Investigate what ran gc during the spawn (likely a jj/git operation in the colocated checkout) and move
> it to a moment when the user is not waiting.

The maintainer's intent, from the planning page: "The whole point of the entire system was that after you launch a
session, we would run refreshes and potential GC and stuff in the background. ... The whole point was to not do it
synchronously. The whole point was to do it asynchronously immediately after a session is started."

What the planning research found (line numbers approximate, main at 66e77cf4):

- **No jj runs anywhere**, and plain folder launches run no git. A managed-checkout launch never touches the user's own
  repositories: it clones from GitHub through a per-host bare cache (`<state_dir>/repo-cache/<owner>/<name>.git`).
- **The launch's own cache refresh runs git's cleanup in the foreground on purpose.** `clone_from_repo_cache`
  (`crates/farhelm-supervisor/src/launch.rs` around 2121-2178) takes the cache lock, then runs
  `git -c gc.autoDetach=false -c maintenance.autoDetach=false -c gc.autoPackLimit=10 --git-dir <cache> fetch --prune ...`
  and `git -c core.commitGraph=false clone --reference <cache> --dissociate`. Fetch runs git's automatic maintenance, so
  once more than 10 packs accumulate, `gc --auto` runs in the session's terminal and the launch waits ("Auto packing the
  repository for optimum performance"). SPEC_impl.md (around 3945-3954) specifies this: "It runs in the foreground so it
  cannot outlive the lock; an occasional launch is slower while the cache consolidates." So does
  `docs/github-checkouts.md` (around 75-83). The test `d2_real_git_consolidates_accumulated_cache_packs` (`launch.rs`
  around 6298) asserts that a normal launch consolidates.
- **The background refresh exists but never consolidates ahead of time.** The spare builder (`service/spare_builder.rs`,
  `spare_build.rs`) registers a trigger after every successful fresh create into a managed checkout, spares on or off
  (`core.rs` around 11605-11620, 12346-12350), coalesces per repository (around 70-94), waits for the launch's clone to
  finish (`next`/`clone_finished`, around 221-292), and takes the cache lock without blocking the visible launch
  (`wait_cache`, around 441-465). But it builds only when "keep a spare" is on and the slot is empty, and it calls the
  same `clone_from_repo_cache` with the same threshold, so the cache can sit at exactly 10 packs after it and the next
  refresh that adds one pack does the cleanup. With spares off, `next` returns the registration at once, without waiting
  for the clone (`if !desired.trigger.enabled { return Some(desired); }`, around 230-232), and only a cheap discard
  runs. Git runs only through a login-shell helper under an owner tether and deadline (`spare_build.rs` around 1-6 and
  71-80, `run_guard` in `spare_builder.rs`).
- **A launch refreshes the cache itself whenever no spare is ready:** the previous launch took it and its replacement is
  not built yet, two launches at once, a supervisor restart that dropped a pending rebuild (nothing rebuilds at
  startup), a failed rebuild (nothing retries), the first launch of a repository, a declined take, or spares off.
- **The spare take** (`spare_take.rs` around 90-99 and 177-181) runs `fetch` in the spare and `merge --ff-only` in the
  new checkout with git's defaults (detached cleanup, high thresholds); it practically never triggers, but it is still a
  command a launch waits on.
- **History:** the foreground consolidation came from the cached-clone speedup (023b8dc6, #2508: "Occasional
  consolidation trades one slower launch for faster later checkouts"), before spares existed; the spare plan (#2511)
  then made the builder an exact copy of that refresh. The maintainer's intent that cleanup never happens while the user
  waits was never written into either plan.
- **Most likely incident:** a launch with no ready spare refreshed the cache, kept an eleventh pack, and git cleaned up
  in front of the user (planning research, about 75% confident).

### Acceptance criteria

- **No automatic git cleanup on anything a launch waits on:** the launch's cache refresh, its clone, the spare take's
  fetch and its `merge --ff-only` all run with git's automatic gc and maintenance off
  (`-c gc.auto=0 -c maintenance.auto=false`, or the equivalent flags of the installed git), and none of them carries the
  foreground consolidation settings any more.
- **The cache consolidates in the background after every launch.** After every successful fresh create into a managed
  checkout, spares on or off and whether or not a spare was taken, the host consolidates that repository's cache with
  today's rule (`git gc --auto` with `gc.autoPackLimit=10`, in the foreground of the background step, under the cache
  lock so it cannot outlive it), once the launch's clone stage has finished, through the spare builder's existing
  registration and the existing login-shell helper. When a spare build runs anyway, its own cache refresh keeps
  consolidating as today; do not run the cleanup twice for one launch.
- **Waits for the clone in both modes.** The background step never takes the cache lock before the launch's clone stage
  ends, with spares off as well as on.
- **Accepted residual:** a launch that arrives while the background cleanup holds the cache lock still waits for it. If
  it is simple, the cleanup may skip itself when a launch is already waiting for the lock; do not build more than that.
- **Unchanged:** what the user's own checkouts do with git afterwards; the cache's location, lock, eviction and the
  spare rules; the session terminal's visible output except that the cleanup's lines no longer appear there.
- SPEC_impl.md (around 3945-3954) and `docs/github-checkouts.md` (around 75-83) describe the new rule; SPEC.md's
  managed-checkout text (around 1348-1384) is updated where it describes what the clone stage or the builder runs.
- Tests: `d2_real_git_consolidates_accumulated_cache_packs` is reworked to assert that a launch's refresh with more than
  10 packs does not consolidate and that the background step after it does; a test that the background step waits for
  the clone with spares off; a test that the take path's git commands carry the overrides. Keep using real git where the
  current tests do.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-07, on the planning page:

- **Where the cleanup happens: in the background right after every launch, spares on or off** (the page's recommended
  option, chosen).
- **A launch that arrives while that background cleanup holds the cache lock still waits.** In the maintainer's words:
  "yes this is imperfect but intentional." The executor may make the cleanup skip itself when a launch is waiting, if
  that is simple.
- **Keep today's consolidation rule** (planner decision after the planning review): move the existing cleanup off the
  launch rather than change its policy; no new repack strategy.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's intent quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's managed checkouts and spares (around 1348-1384); SPEC_impl.md's repository
cache (around 3945-3954) and the spare builder's sections; `docs/github-checkouts.md`; the supervisor's rule that git
runs through the login-shell helper; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog
fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and
`scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Give `clone_from_repo_cache` a parameter for whether the refresh consolidates, rather than a second copy: the launch
  passes "no", the spare build passes "yes".
- Put the consolidation-only step in the builder's `attempt` (`spare_builder.rs` around 308) as a mode of the existing
  helper (a build spec with no target), used when no spare build runs for that registration.
- Low-priority scheduling (nice) is not required; keep it if the helper already applies it.

## Outline

One PR:

1. `plan/no-gc-at-launch/01-no-gc-at-launch`: the overrides on every waited-on git command, the consolidation-only
   background step and the clone wait with spares off, the reworked and new tests, SPEC_impl.md, SPEC.md where needed,
   `docs/github-checkouts.md`, the changelog fragment and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy, and,
through `python3 scripts/record-test-run.py` with the pinned nextest setup, the supervisor's `launch`, `spare_take`,
`spare_build` and `spare_builder` unit tests and the e2e tests for GitHub checkouts and spares;
`python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares, if tests change.

## Fallbacks

Examples of departures that need the scope reassessment: a new background task or queue beside the spare builder, a
different consolidation policy (a geometric repack, say), consolidating the user's checkouts, rebuilding spares at
startup, or making launches never wait on the cache lock.

Decisions to log: how the launch and the builder share `clone_from_repo_cache`, how the step avoids running twice for
one launch, whether the skip-when-waiting optimization was built, every test changed and why, and every review finding
you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-no-gc-at-launch-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/no-gc-at-launch/<nn>-<short-name>`, as
  the outline names them.
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
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.

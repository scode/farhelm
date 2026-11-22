# Refuse deflake bases older than the current sweep

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Keep deflake evidence tied to the selected revision's prerequisites.** Running the current deflake driver with an
> older `--base` uses that revision's recorder with today's battery list and system tmux. Before the pinned tmux
> removal, the driver ran the selected revision's `scripts/build-pinned-tmux-ci.sh` and passed `--tmux required` for
> workspace nextest, workspace doctests, Playwright and the old desktop smoke test. It passed `--tmux none` for desktop
> nextest and desktop doctests. Without required mode the historical recorder warns and continues after a mismatch.
> Decide which historical batteries and bases the current driver supports. For a pre-removal base, verify either
> enforcement of its pin and phase requirements or refusal with an explanation before any tests run. For a current base,
> verify the system-tmux path. Plan: `plans/queue/deflake-historical-base-prereqs.md`.

### Where it stands (from planning research at main b34ceee9f)

- A deflake run sweeps a base revision (`--base REV`, default `main`; `deflake/bin/deflake`, about line 1529) in a
  detached git worktree. It runs the current driver's battery list (`phases()`, about lines 488-535) through that
  revision's recorder, `scripts/record-test-run.py` in the worktree (`recorder_argv`, about lines 843-849).
- `prepare()` (about lines 1094-1170) resolves the base with `jj --ignore-working-copy log -r <base> -T commit_id` in
  the checkout (about lines 1110-1116), checks the worktree out at that commit, and runs the prepare steps: musl-tools
  if missing, the pinned nextest, `tmux -V` (any working tmux), and a cold `cargo build`. A failure there is a
  `prepare-failed` event, which `deflake/AGENTS.md` already treats as a tooling problem.
- `cmd_start` (about lines 1332-1371) runs synchronously before the daemon detaches: it prepares the state root, probes
  `daemon.lock` (refusing when another sweep's daemon holds it), creates the run directory and the `current/` pointer,
  then starts the daemon. Its exit status is what the agent sees.
- The sweep changed shape in three commits: `d751ff163` (2026-11-12) replaced the revision's pinned tmux build and
  `--tmux required` / `--tmux none` with today's `tmux -V`; `6ef88c9a4` and `1eb4e3751` (both 2026-11-13) removed the
  Playwright, JS unit and desktop smoke batteries and the dx, `npm ci` and browser-install prepare steps, and moved the
  desktop batteries from `-p farhelm-ui --features desktop` to `-p farhelm-desktop`. The only later driver change,
  `f1cb3c1f0` (2026-11-20), touched the state root's ownership check. Later recorder changes (`f1cb3c1f0`, `6a634570a`)
  did not change its command line or its evidence announcement.
- So a base before `1eb4e3751` gets batteries it never ran, loses batteries it did run, and runs under a recorder that
  only warns on the wrong tmux: a sweep can finish clean while covering less than that revision's own sweep did.
- Nothing in the repository passes `--base` besides the CLI definition; `deflake/AGENTS.md`, "Before starting", says the
  sweep runs against `main` unless the user names another base. `deflake/SPEC.md` (about line 10) says a run "runs
  against a fixed base commit (main by default)".
- `deflake/test-deflake.py` unit-tests the driver's pure parts, loading the extensionless script as a module and
  injecting collaborators with `mock.patch.object`. `deflake/EVAL.md` is the required end-to-end check for any change to
  the driver or `deflake/AGENTS.md` (`deflake/AGENTS.md`, "Changing the deflake tooling"); its clone is a full clone of
  your checkout (`deflake/eval-setup.sh`), so old commits are present there.
- The inventories already list `deflake/test-deflake.py` and EVAL (`.agents/validation.md`, about lines 1048-1050;
  `.agents/run-all-tests.md`, about line 107), and no phase changes, so no inventory update is needed.

### Acceptance criteria

1. The driver has one named constant, the oldest base it can sweep, set to `1eb4e3751` (the full commit id), with a
   comment saying why it sits there and that it is raised whenever a driver change makes older bases unsweepable.
2. `deflake start` refuses a base that is not a descendant of the floor (or the floor itself), and a base that does not
   resolve, before it prepares the state root, probes the daemon lock, or creates a run directory or `current/` pointer:
   it exits non-zero with an explanation naming the base, the commit it resolved to, the floor and why the floor exists.
   A side branch forked before the floor is refused too.
3. `start` and `prepare()` share one helper that both resolves the base to a commit and applies the floor check, so
   neither keeps its own copy of the resolution. In `prepare()` a refusal fails as `prepare-failed` with the same
   explanation, so a repeat run or any other path into prepare cannot sweep an old base; prepare checks out the commit
   that helper returned.
4. A current base still sweeps exactly as today, including the system-tmux prepare step; the report cites the EVAL run's
   `phases/prepare-tmux.log` (the system `tmux -V` step passing) as the evidence the TODO entry asks for.
5. `deflake/SPEC.md`, the module docstring, the `start` help text and `deflake/AGENTS.md` say there is a floor; the
   "Changing the deflake tooling" reading list entry gains the rule to raise the floor when phases, prepare steps or the
   recorder interface the driver depends on change incompatibly. No `/usr/bin/tmux` tightening.
6. Unit tests in `deflake/test-deflake.py` cover the predicate against a temporary git repository with an injected
   floor: a descendant accepted, the floor itself accepted, the floor's parent refused, an unrelated branch refused, an
   unknown commit refused, the refusal text naming the floor.
7. A live refusal is shown inside the throwaway eval checkout, and the required `deflake/EVAL.md` pass is clean.
8. The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Refuse, do not restore:** bases older than a floor are refused with an explanation, rather than restoring historical
  prerequisites (the pinned tmux build, `--tmux required`, the browser and webview batteries and their prepare steps).
- **The floor:** `1eb4e3751` (2026-11-13), the last change to the battery list and prepare steps, which also covers the
  tmux removal a day earlier.
- **Mechanism:** a git ancestry check, run in `start` before any lock probe or run directory, and again in `prepare`.
- **Keeping it current:** a comment on the constant, and a rule in `deflake/AGENTS.md`, "Changing the deflake tooling",
  to raise it when phases, prepare steps or the recorder interface change.
- **No `/usr/bin/tmux` tightening** of the current prepare step.
- **Validation:** unit tests in `deflake/test-deflake.py`, a live refusal inside the throwaway eval checkout, and the
  required `deflake/EVAL.md` pass, under the shared-machine rules: one sweep per machine; never stop another agent's
  sweep; unattended, wait or report.
- **Shape:** one `chore:` PR, no changelog fragment (maintainer tooling only; nothing shipped changes).
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- `deflake/SPEC.md` (this adds a floor to "runs against a fixed base commit"; it does not contradict it) and
  `deflake/AGENTS.md`, "Changing the deflake tooling": read `deflake/EVAL.md` and `deflake/SPEC.md` in full before the
  change, and validate it with `deflake/EVAL.md` before calling it done.
- `deflake/AGENTS.md`, "Before starting": only one sweep runs on the machine; another agent's sweep is never stopped.
- Root `AGENTS.md`, "Sharing the machine with other agents", "Agent scratch space" (the eval directory goes in your
  scratch directory), "Conventional Commits", "Changelog fragments", "Tests and validation", and
  `python3 scripts/check-agents-md.py` for the instruction file change.
- `.agents/test-authoring.md` for the new tests.

**Planner proposals:**

- One helper, used by both `start` and `prepare()`, that resolves the base (moving prepare's existing jj call and its
  40-hex check into it, rather than copying them) and then applies an ancestry predicate taking the floor as a parameter
  (so the test injects one), which runs `git merge-base --is-ancestor` in the checkout. An unknown floor commit (a
  checkout without that history) is refused with its own explanation rather than treated as a pass.
- The refusal text, roughly: base X (commit) predates 1eb4e3751, 2026-11-13, the last change to the sweep's batteries
  and prerequisites; this driver cannot sweep it.
- In `deflake/AGENTS.md`, one sentence where the base is mentioned ("Before starting" or "The loop") saying what a
  refused `start` means for the agent: tell the user and stop. Keep it short: the low-power EVAL delegate reads it.

## Outline

1. `plan/deflake-historical-base-prereqs/01-refuse-old-bases`: the constant, the predicate and its two call sites, the
   unit tests, the `deflake/SPEC.md`, docstring, help text and `deflake/AGENTS.md` updates, and the TODO entry's
   removal. `chore:`, no changelog fragment.

## Fallbacks

- If you find a driver or recorder interface change after `1eb4e3751` that makes bases between it and today unsweepable,
  raise the floor to that change instead, and log the commit and why.
- Live refusal: run `deflake/bin/deflake start --profile eval --base <a pre-floor commit>` only inside the eval checkout
  `deflake/eval-setup.sh` makes (never in your own checkout, where a broken check would start a real sweep), and confirm
  it exits non-zero with the explanation and leaves no run directory or `current/` pointer for that checkout. The check
  runs before the daemon lock probe, so another agent's sweep does not get in its way.
- The EVAL pass needs the machine's one sweep slot. Before starting it, check whether another agent's sweep holds the
  daemon lock (probe the lock without holding it beyond an instant, or with `deflake/bin/deflake status` where that
  answers). If it is held, never stop it: wait, checking no more often than every 15 minutes, for up to 4 hours. If it
  is still held after that, push the PR with everything else done, and block (`plans/.agents/executing.md`, "Executing
  one plan", step 10) with the question whether to deliver without the EVAL pass or keep waiting, since EVAL is a
  required check.
- An EVAL pass that is not clean is a driver or instruction bug to fix and re-run per `deflake/EVAL.md`, step 4; do not
  deliver around it.
- Clean up every EVAL pass per `deflake/EVAL.md`, step 6, including the sweep worktree and its lock only once the run is
  over.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-deflake-historical-base-prereqs-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-deflake-historical-base-prereqs-watchdog.json` beside
it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent
scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

For this plan: `python3 -B deflake/test-deflake.py`, the live refusal and the EVAL pass (Fallbacks above),
`python3 scripts/check-agents-md.py`, and `dprint check` on the changed Markdown. The EVAL pass pays a cold cargo build
of about ten minutes on the shared CPU; run it once the change is otherwise final, so one clean pass covers it.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

One more exemption: the `deflake/EVAL.md` delegate. That procedure requires a fresh, low-power agent (a Sonnet-class
model at medium effort) as its subject, because the eval tests whether the instructions are unambiguous for such a
model; it is the check's own calibration, not a unit of your work. Launch it as `deflake/EVAL.md`, "Delegate prompt",
says, natively where your harness offers such a model and otherwise through `$scode-harness-shellout`, and do not coach
it.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/deflake-historical-base-prereqs/01-refuse-old-bases`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Keep deflake evidence tied to the selected
  revision's prerequisites"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

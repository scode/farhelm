# Activate previous terminal tab on tab close

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says, and
where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one `TODO.md` entry (bucket "Immediate"). Verbatim:

> **Remember the order tabs were opened in.** Remember order of tabs being open. So if you have an agent open, then you
> open a terminal, then you open a second terminal, then you close the second terminal, the first terminal should be
> made active - not the agent.

### Acceptance criteria

1. **Previous tab in open/strip order becomes active on tab closure:**
   - In `crates/gui/farhelm-gui/src/terminal/set.rs`, when the currently active shell tab closes (whether from an
     explicit user Close in `TerminalSet::close_tab` or from a shell exit / external close reconciled in
     `TerminalSet::follow_tabs`), select the preceding surviving tab in creation/strip order (`self.tabs`, which the
     helm already supplies in monotonic creation-stamp order): index `i - 1` when `i > 0`, or the session's main
     agent/shell terminal (`None`) when `i == 0` or no earlier surviving tab exists.
   - Note that `follow_tabs` currently moves `self.tabs` into a `HashMap` (`old`) before checking whether `self.chosen`
     vanished, discarding the prior strip order; compute the predecessor from the prior order before moving `self.tabs`
     into `old`.
   - Example: Agent → open Terminal 1 → open Terminal 2 → close Terminal 2 activates Terminal 1; closing Terminal 1
     activates Agent. Opening T1, T2, T3 and closing T2 activates T1; closing T1 activates Agent.
   - Closing a non-active tab leaves the currently active tab unchanged.
2. **Synchronize remembered terminal (`ShownTerminals`) and handle concurrent closes safely:**
   - Update `ShownTerminals` (`set.rs`) whenever the active tab changes on close or external removal (inserting the new
     fallback tab id, or removing the entry when falling back to the main agent tab) so switching to another session and
     returning remembers the fallback tab rather than looking up the deleted tab and jumping back to Agent.
   - In `close_tab`, because the switch happens on `DELETE` completion before the roster push removes the closed tab
     from `self.tabs` and sibling tabs can close concurrently under `OpKey::tab`, track confirmed-closed tab IDs on
     `TerminalSet` (pruned against `listed` in `follow_tabs`) and exclude them when picking the predecessor so a close
     never selects a tab that just closed.
   - Preserve existing focus-ownership rules: focus moves to the fallback terminal only when the closed terminal held
     focus, leaving open dialogs or inline header prompts undisturbed.
   - Keep `follow_tab_attach` refusal handling intact (a restored tab whose initial attach is `Refused` still falls back
     to the main terminal without wiping `ShownTerminals`).
3. **Specs, comments, and tests:**
   - Update `SPEC.md` ("Session view", which currently does not state the previous-tab activation rule, and "Cases that
     settled disputed points", where an exiting shell tab currently states that the session's agent tab takes focus) to
     reflect the previous-tab-in-open-order rule for all tab closures.
   - Update module doc comments in `crates/gui/farhelm-gui/src/terminal/set.rs` and
     `crates/gui/farhelm-gui/src/session_pane.rs` where they repeat the old agent-tab fallback rule.
   - Add/update unit tests in `crates/gui/farhelm-gui/src/terminal/set/tests.rs` covering:
     - explicit close of a second terminal tab activating the first terminal tab (and updating `ShownTerminals` across
       session switch/remount),
     - closing the first terminal tab activating the main agent tab,
     - closing a middle tab activating its predecessor,
     - closing a non-active tab keeping the active tab,
     - an exiting/reaped or externally closed second tab activating the first terminal tab (and taking focus when the
       closed tab held focus, while leaving a header question or dialog focused when it did not),
     - concurrent closes of sibling tabs not activating an already-closed sibling.
   - Remove the covered `TODO.md` entry in the PR.

## Decisions already made

### Maintainer decisions (binding)

- **Fallback order:** Previous tab in open/strip order (index `i - 1`, or Agent when `i == 0`), not an MRU activation
  stack.
- **External close / shell exit:** Use the same previous-tab rule for all tab closures (both explicit Close and shell
  exit / external close), updating `SPEC.md` accordingly.
- **Review gate:** Single fresh-context agent with the general charter on `Opus 5.5` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- `SPEC_impl.md` guarantees `SessionRow.info.tabs` arrives in monotonic creation-stamp order; rely on that existing
  order rather than inventing a parallel order store or persisted disk state.
- `AGENTS.md`: Never mutate process environment variables in tests; never touch the live Farhelm install; annotate every
  test sleep with `// sleep-ok:` passing `scripts/check-test-sleeps.py`.

### Planner-chosen implementation shape

- **Reuse `self.tabs`'s existing creation-stamp order and `ShownTerminals` in
  `crates/gui/farhelm-gui/src/terminal/set.rs`** with one small helper (e.g. `previous_surviving_tab`) shared by
  `close_tab` and `follow_tabs`, plus a plain set of confirmed-closed tab IDs on `TerminalSet` (pruned against `listed`
  in `follow_tabs`) so concurrent `DELETE` completions never pick an already-closed tab. No new persistent state,
  protocol field, or UI layout change is needed.

## Outline

### Existing mechanisms reused

- `crates/gui/farhelm-gui/src/terminal/set.rs` (`TerminalSet`, `ShownTerminals`, `close_tab`, `follow_tabs`, `shown`,
  `home`).
- `crates/gui/farhelm-gui/src/session_pane.rs` (observes `TerminalSet` and hands focus via `focus::set_home`).
- `crates/gui/farhelm-gui/src/terminal/set/tests.rs` (existing tab close, reap, and focus tests).

### PR stack

1. **`plan/tab-close-activation-order/01-activate-previous-tab-on-close`** — Pick the preceding surviving tab in
   creation/strip order on both explicit close and external/reaped tab removal (computing the predecessor before
   `follow_tabs` consumes `self.tabs`), keep `ShownTerminals` synchronized, track confirmed-closed tab IDs until the
   roster push catches up, update `SPEC.md`, stale module doc comments, and `set/tests.rs`, add the changelog fragment,
   and remove the covered `TODO.md` entry.

## Fallbacks

- If a fallback candidate tab is unattached (`terminal: None`, e.g. past `MAX_ATTACHED_TABS` = 32), check how `shown()`
  and `home()` handle selecting it and choose the cleanest behavior consistent with existing tab-selection semantics,
  logging the choice as a `DECISION`.
- Scope-reassessment triggers (examples, not a blacklist): adding persistent disk storage for tab history, adding a new
  helm or supervisor protocol field for tab order, or changing tab strip rendering/goldens.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tab-close-activation-order-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-tab-close-activation-order-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- Targeted Rust tests (through `record-test-run.py` with pinned nextest on `PATH`):
  - `cargo nextest run -p farhelm-gui --lib -E 'test(/^terminal::set::/) | test(/^session_pane::/)'`
- Formatting, lints, and checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy -p farhelm-gui --all-targets -- -D warnings`
  - `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`
  - `dprint check` on changed Markdown files
  - `python3 releasing/check-changelog.py format`
  - `python3 scripts/check-agents-md.py` (if headings or instruction files change)

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the `TODO.md` entry this plan covers ("Remember the order tabs were opened in."), as
  root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred items added to `TODO.md` go under
  `## Plan follow-ups`.
- Commit messages, PR titles, and changelog fragments follow root `AGENTS.md`, "Conventional Commits" and "Changelog
  fragments". This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you (`plans/.agents/executing.md`, "Executing one plan", step 8); never edit that section.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `Opus 5.5` at `high` effort,
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

Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular concurrent-close exclusion before roster push, handling of unattached tabs if
applicable, and any review finding you decided not to follow. The maintainer will ask for these later.

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
DECISION, or it is a block. Things the maintainer should know and possible follow-ups go into the report as statements.

### Done criterion

The goal is achieved when every PR in the linear stack exists on GitHub as a draft PR and has passed its review gate,
and the latest `## Decisions` entry, if any, is satisfied. Delivering the report is your closing step per
`plans/.agents/executing.md`, "Executing one plan" (step 11), not part of the goal.

## Release note

No release note needed.

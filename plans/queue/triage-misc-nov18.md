# Five small outcomes: detach input order, launching containers in the leftovers list, a test fixture, two spec notes

Written against main at f2fc9607 on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute five unrelated triage outcomes recorded on 2026-11-18, per `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes" (one PR per outcome; a `fix spec+code` outcome is never split). Their `TRIAGE_OUTCOMES.md`
entries are the binding statement of what to do; read each in full from `main@origin`, together with its feedback file
under `review_feedback_queue/`:

1. `trailing-natural-detach-unlocks-before-cancelling-unpolled-grid-input.md` (`fix code`, recorded without asking under
   the clearly-settled rule)
2. `container-leftover-delete-races-first-session.md` (`fix code`)
3. `trailing-omp-admission-host-fixture-retains-unowned-bare-pgid.md` (`fix code`, test tooling)
4. `wayland-enter-overtakes-pending-text-paste.md` (`fix spec`)
5. `trailing-restart-seed-history-promoted-into-launch-excerpt.md` (`fix spec`)

### What each PR does

1. **Natural detach stops the old viewer's input first.** `detach_naturally` in
   `crates/farhelm-supervisor/src/service/connection.rs` (around 1487-1518) releases the attachments lock before the old
   attachment, whose input side's drop cancels its pump, is dropped; `detach_stalled` (around 1449-1458) drops it first.
   Make natural detach drop the old attachment's input side before releasing the lock, as SPEC_impl.md requires ("An
   attachment that ends drops the input its pump had not handed to the runtime and stops the pump at once; what the pump
   handed over is written before anything a replacement attachment sends"). About 5 lines; a test only if it stays
   small. `fix:` with a changelog fragment. The ledger's 100-line size gate applies: over it, block and bring it back.
2. **The leftovers list leaves out containers still launching.** Settings → Containers not on this helm lists any
   row-backed container with no cached sessions (`crates/farhelm-helm/src/environment/podman.rs`, around 840-860), so a
   container whose first launch is still running appears there, and confirming its Delete after the launch finishes
   removes the new session. SPEC.md "Container sessions" limits that list to failed cleanup and interrupted launches.
   Leave containers whose first launch is in progress out of the list, using the launch state the helm already holds,
   with a test. About 15-30 lines. Follow the provider map in `crates/farhelm-helm/src/environment/mod.rs` (root
   `AGENTS.md`, "Code rules"): no comparisons of individual providers in shared code. `fix:` with a changelog fragment.
3. **The OMP admission fixture signals only its own process group.** A supervisor test fixture
   (`crates/farhelm-supervisor/src/service/core.rs`, tests, around 22487-22754 and its Drop around 23185-23199) keeps
   only the process-group number of host-run fake agents and SIGKILLs that group at teardown. Record the group leader's
   identity (process number and start time, as the supervisor's own process checks do) and signal the group only while
   that leader still matches; otherwise leave the fake agent's sleep to end on its own, as the fixture's docs already
   accept. About 15-30 lines of test code. `test:`, no fragment. Read `.agents/test-authoring.md` and run
   `python -B scripts/check-test-sleeps.py` as root `AGENTS.md` says for Rust test changes.
4. **Wayland paste order, as a spec note.** SPEC.md, beside its rule that Farhelm adds no complexity for a broken, hung
   or slow clipboard ("Terminal experience"), says that on Wayland, input typed while a paste is still being fetched
   from the clipboard may reach the terminal before the paste. No code change. `docs:`.
5. **Restart's launch-error quote, as a spec note.** SPEC_impl.md (around 2362-2364: "a Restart's new host starts with a
   blank screen, so no quote reaches back into the previous run") and the matching doc comment in
   `crates/farhelm-supervisor/src/service/launch_artifacts.rs` (around 52-55) are inaccurate once the restarted terminal
   grows before the failure is quoted: growing pulls the carried-over history into view. Correct both to say that the
   quote may then include lines of the same session's previous run. No behavior change. `docs:`.

Each PR removes its feedback file and index line and moves its ledger entry to the archive.

### Acceptance criteria

- Five PRs, each meeting its ledger entry's completion criteria and recording its execution.
- Item 1: natural detach drops the old input side before the lock is released.
- Item 2: a container whose first launch is in progress is not listed; failed cleanup and interrupted launches still
  are.
- Item 3: the fixture never signals a group whose leader's identity no longer matches.

## Decisions already made

- **The outcomes:** item 1 recorded under the 2026-10-19 clearly-settled rule; item 4 from the maintainer's pick on Q3
  of the 2026-11-18 highest-bucket triage page ("spec note for paste order"); items 2, 3 and 5 from accepting that
  page's Q7 batch of recommendations.
- **Grouping:** the maintainer approved these five as one plan on the 2026-11-18 planning page; each is still its own
  PR.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **No release note** (the maintainer, 2026-11-18: every plan from this triage needs none). The `fix:` PRs still leave
  their changelog fragments.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the five `TRIAGE_OUTCOMES.md` entries and the Decisions above.

**Binding repository constraints:** SPEC_impl.md's attachment input rule (item 1); SPEC.md "Container sessions" (item
2); the environment provider map (item 2); SPEC.md's slow-clipboard rule (item 4); `.agents/test-authoring.md` and the
test-sleep check (item 3); `review_feedback_queue/.agents/execute-outcomes.md`; root `AGENTS.md`, "Tests and
validation", "Changelog fragments", "Conventional Commits", "Code rules".

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): item 2 uses launch state the helm
already holds rather than a new record; item 3 reuses the supervisor's existing process identity check rather than a new
one.

## Outline

Five PRs, bottom to top:

1. `plan/triage-misc-nov18/01-natural-detach-input-order`
2. `plan/triage-misc-nov18/02-leftovers-skip-launching-containers`
3. `plan/triage-misc-nov18/03-omp-fixture-group-identity`
4. `plan/triage-misc-nov18/04-wayland-paste-order-spec`
5. `plan/triage-misc-nov18/05-restart-quote-spec`

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy on `farhelm-supervisor`
and `farhelm-helm`, the grid streams selection for item 1, the helm's Podman leftover tests for item 2 and the touched
fixture's tests for item 3 through `python3 scripts/record-test-run.py`, the test-sleep check, and `dprint check` on the
spec files. PRs 4 and 5 are documentation and queue bookkeeping only (item 5's comment edit has no doctest role): no
runtime tests.

## Fallbacks

Examples of departures that need the scope reassessment: a new durable launch record for item 2; changing how Restart
seeds history or how the quote is taken for item 5; holding typed keys behind a paste for item 4.

Decisions to log: item 2's source of launch state, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-triage-misc-nov18-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-triage-misc-nov18-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests on the machine you run on, as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`. The maintainer chose this on 2026-11-18; do not use the boat sandboxes
(`.agents/boat.md`) for this plan.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (one of its remotes
points at that GitHub repository; if not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill
with the log file's absolute path. If the log exists, read it and resume where the previous session left off,
cross-checking it against reality (the jj graph, bookmarks and open PRs) rather than starting over. If it does not
exist, this is a fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the
resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

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
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

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

- One reviewable commit, stable bookmark and draft PR per outcome, in a single linear stack, using the `jjstack` skill,
  as `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes", requires (the outline above names
  any agreed exception). The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/triage-misc-nov18/<nn>-<short-name>`, as the outline names them.
- Each PR carries its outcome's agreed change, removes the outcome's feedback file and its
  `review_feedback_queue/INDEX.md` line, and moves the outcome's `TRIAGE_OUTCOMES.md` entry, with its final Execution
  field, to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (`review_feedback_queue/AGENTS.md`, "Lifecycle"). Record the change
  ID and bookmark in the Execution field before creating the PR, then add the PR's URL to the same change and update the
  PR; never make a separate bookkeeping PR.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"), whatever this plan's Release note section says.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline, and the outcome's `TRIAGE_OUTCOMES.md` entry). When a PR changes tests or their fixtures,
include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
working log with the true value; never edit this plan file. If current code or specs invalidate an outcome's decision
(`review_feedback_queue/.agents/execute-outcomes.md`), that outcome is a block, not something to reinterpret.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and each having recorded its outcome's
execution and moved its ledger entry to the archive. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

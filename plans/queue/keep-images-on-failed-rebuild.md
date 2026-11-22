# Keep a container recipe's working images when Rebuild fails

Written against main at d6157939f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Keep working container images when Rebuild fails.** Rebuild currently retires a recipe's Podman software and
> prepared images before building replacements (`crates/farhelm-helm/src/environment/podman/build.rs`). A package or
> installer outage can leave a previously usable recipe unable to start new sessions. Force actual rebuilding with a
> flag, retain the last usable images until replacement succeeds, and keep the ordinary freshness rules for later
> launches. Launches for the same recipe and place must still join an active Rebuild and receive its outcome, even while
> old images remain recorded. Coordinate source selection with the job registry and release any source guard before
> awaiting the rebuild's publication. Test both a failure with a usable fallback and a launch concurrent with Rebuild.
> Deferred from the Podman triage plan because preserving active-job joining exceeds the original size gate.

In product terms: a Podman container recipe has two images per container host, the software image (base, packages,
Claude Code, Codex) and the prepared image (the software image with Farhelm installed), and new sessions start from the
prepared image. Settings → Recipes offers Rebuild per host. Today Rebuild discards both working images before building,
so a failed build leaves the recipe unable to start sessions at that host until a later build succeeds. Tensorlake
recipes already keep their old snapshot until a Rebuild succeeds. The 2026-11-21 Podman triage plan deferred this (PR
#4490) because discarding early is also what makes a launch during a Rebuild wait for it: simply keeping the images
would let that launch start on stale ones.

### Where it stands (from planning research at main d6157939f; line numbers approximate)

- **Admission.** `rebuild_recipe` (`environment/podman.rs`, about 1050-1084) calls
  `start_podman(name, place, rebuild=true, …)` in the shared recipe build registry (`tensorlake/build.rs`, about
  703-756), which inserts a `PodmanJob { rebuild: true }` per recipe and place. A Rebuild cannot join an ordinary
  preparation job and is refused as busy; launches can join a Rebuild. `podman_job_active` (about 822) already looks a
  job up.
- **Early retirement.** `environment/podman/build.rs`, about 254-266: under `lock_podman_use`, Rebuild moves both images
  into `retired_images`. That defeats two reuse shortcuts, `fresh_image` (about 310) and software reuse while
  `state.image` exists (about 133-137), and it makes launches join (below).
- **Publication.** The software image is published first (about 221-229), the old one pushed to the retired list; the
  prepared image is published after a same-source check (about 317-326 and again at about 380-398), then `retire_images`
  (about 400). Newly committed images sit in `retired_images` until published (`podman.rs`, about 563-606), so an
  unpublished image is already owned.
- **On failure,** `prepare_images` cleans up helpers, runs `retire_images` and records `last_failure` (about 271-291).
  Because the old images were retired at admission, they are deleted here unless a container still uses them. That is
  the bug.
- **Source selection.** `ensure_prepared` (`podman.rs`, about 1134-1178) calls `fresh_image` without consulting the
  registry. With no fresh image it calls `start_podman(false)`, which joins any running job, and waits on its result; a
  failure becomes the launch's error. With one, it takes `lock_podman_use` and rechecks. That `OwnedMutexGuard` (the
  "source guard") travels inside `PreparedContainer` (about 75-77) and is dropped at the container's first Running
  (about 1315-1317). Rebuild's publication and `retire_images` take the same mutex, so waiting on a Rebuild while
  holding it deadlocks.
- **Tensorlake's precedent** (`tensorlake/build.rs`, `ensure_snapshot`, about 1053-1095): it uses the recorded snapshot
  only when no build is running, otherwise joins; a joined launch that sees a failure fails with `last_failure`; later
  launches use the old snapshot. Its `is_building` reads a different map and is not reusable here.
- **What the UI shows.** While a Rebuild runs, building takes priority over ready, and `last_failure` is cleared when
  the job starts (`build.rs`, about 252). After a failure with fresh old images, the place already reads as ready with
  the failure as its reason (`places.rs`, about 212-214; `placement.rs`, about 246-247).
- **Tests.** There is no direct Podman Rebuild test. The closest are `shared_admission_joins_lazy_work…`
  (`environment/podman/tests.rs`, about 841) and `absent_helper_create_allows_immediate_rebuild…`
  (`environment/podman/failure_tests.rs`, about 1225). The Podman fake (`failure_tests.rs`, about 156-256; fixtures in
  `provisioning/session_launch_failure_tests.rs`, about 149-187) cannot run a full preparation end to end: one fixed ID
  for every container and image, one shared image-exists flag, panics on pull, commit, stop and scripts, and
  user-manager setup fails for lack of a provisioning transport.

### Acceptance criteria

1. Rebuild no longer discards the current images at admission. A Rebuild is forced by passing the existing `rebuild`
   flag down into the preparation and software steps, so it skips the `fresh_image` shortcut and software reuse; no new
   type. Both same-source comparisons (before preparing and at publication) are skipped for a forced run, since the
   registry already runs one job per recipe and place and deletion cancels it first.
2. The new software image stays unpublished (owned, in the retired list) until the prepared image is built. Publication
   is one guarded edit at the existing publication site that records both new images, removes them from the retired list
   and moves the old ones onto it; the old images are then removed once no container uses them, as today. A doc comment
   says why an unpublished image is safe in the retired list (nothing else retires images mid-job).
3. On failure, only the new, unpublished images are cleaned up; the previous images stay recorded and present, and the
   failure is recorded as today. Later launches use the previous images under the ordinary freshness rules.
4. A launch for a recipe and place with a job in progress joins it and receives its outcome, even while the old images
   remain recorded: `ensure_prepared` checks the registry under the source guard, through one accessor that turns
   `podman_job_active` into an `Option` of the job's progress receiver for a job whose result is not yet set (its other
   caller checks `.is_some()`). With a job in progress, it drops the guard before waiting, waits through the same wait
   loop the lazy path uses (one shared helper, not two copies), returns the job's error on failure, and on success takes
   the guard again and uses the fresh image. It never waits while holding the guard. A job whose result is already set
   is not re-joined.
5. After a failed Rebuild, the recipe's status at that host reads "Rebuild failed: <error>. New sessions use the
   previous image." (Decisions). A failed preparation that left no usable images reads as today.
6. Tests, without sleeps (`.agents/test-authoring.md`):
   - failure with a usable fallback: after images exist, a forced Rebuild that fails leaves both previous images
     recorded and present, retires the new image, records the failure, and a later launch uses the previous images;
   - a launch concurrent with a Rebuild, against the registry directly as the shared-admission test does (a gated
     Rebuild job, pre-seeded fresh images, a call to `ensure_prepared`), with a notification from the progress fake so
     the test cannot release the job before the launch has queried: on success the launch gets the new prepared image,
     on failure the Rebuild's error, and the old record stays.
7. SPEC.md, about 5265-5267 and 5337-5339, and SPEC_impl.md, about 6124-6138 and the "Podman places, creation and
   images" section, say that Rebuild replaces both images only on success, that launches during a Rebuild join it, and
   that a failed Rebuild leaves the previous images in use. Doc comments in `build.rs` (module docs, about 121-122 and
   234-235), on `PodmanJob` and on `ensure_prepared` match.
8. The PR carries a changelog fragment and removes the TODO entry.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **A launch that joined a failed Rebuild fails with the Rebuild's error.** Already settled by the TODO entry ("receive
  its outcome"), as Tensorlake does; launching again starts on the previous images.
- **Status wording after a failed Rebuild:** "Rebuild failed: <error>. New sessions use the previous image." The
  maintainer picked this over keeping today's plain error. Put it wherever the helm writes the place's reason for a
  ready place with a recorded failure; if the desktop app writes that text itself, change it there instead, in one
  place.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- SPEC.md and SPEC_impl.md passages named in the acceptance criteria; Tensorlake's rebuild behavior (SPEC.md, about
  574-580) is the model.
- Root `AGENTS.md`, "Code rules": provider decisions live in their implementation or an exhaustive method, per the map
  in `crates/farhelm-helm/src/environment/mod.rs`; read it before changing provider behavior.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and its "Instructions by path" for every path you touch.

**Planner proposals** (from research and an independent planning review):

- Publishing both images together, not software first: a prepared image counts as fresh only if built from the recorded
  software image, so publishing software early would leave a preparation-stage failure with an unusable old prepared
  image.
- The accessor joins any job for the recipe and place, not only a Rebuild: SPEC_impl already says a launch joins any job
  for its recipe and place.
- The concurrent-launch test goes through the registry, not a full build harness. For the failure test, the managed
  Ubuntu base may fail naturally at user-manager setup with the current fixture after committing a new image, which
  covers the contract without a new harness; use it if it does.
- A second failure test failing at the preparation stage is the only one that catches a regression to early publication,
  but it needs most of a full build harness. It is optional: build it only if the harness stays small, and otherwise log
  the decision and name the gap in the report.
- Fragment, `kind: fixed`, along the lines of: "A failed Rebuild of a container recipe no longer leaves you unable to
  start new sessions from it: the recipe keeps its previous images at that host until a Rebuild succeeds."

## Outline

1. `plan/keep-images-on-failed-rebuild/01-keep-images-until-rebuild-succeeds`: the forced flag, deferred publication,
   failure cleanup, the registry accessor and shared wait, `ensure_prepared`'s join, the status wording, tests, spec
   text, a changelog fragment, and the TODO entry's removal. `fix:` (for example
   `fix: keep container recipe images when a rebuild fails`).

## Fallbacks

- If the status wording turns out to be written in several places, change the one that serves both the launcher and
  Settings, and log where.
- If making the concurrent-launch test deterministic needs a gate the fake lacks, add the smallest gate to the fake;
  never a sleep.
- If the diff grows well past the research estimate (about 80-110 lines of product code), run the scope reassessment
  before going on.
- A reviewer finding that asks to fall back to old images for a launch that joined a failed Rebuild is declined by
  decision; log it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-keep-images-on-failed-rebuild-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-keep-images-on-failed-rebuild-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what the PR changes; when the machine has rootless Podman, the
real Podman lifecycle check `.agents/validation.md` lists is worth one run. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/keep-images-on-failed-rebuild/01-keep-images-until-rebuild-succeeds`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Keep working container images when Rebuild
  fails"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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

# Hide unrelated Podman status noise on Tensorlake recipe cards

Written against main at c1d85d8ab on 2026-11-24. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one `TODO.md` entry (bucket "Tensorlake", subsection "v1", "Wave 0 — No-UX-design fixes"). Verbatim:

> **Hide unrelated Podman status noise on Tensorlake recipe cards.** Because Managed Ubuntu is shared with Podman,
> recipe cards show a local Podman check/error line and a `Rebuild` button right above the Tensorlake status lines even
> when the user is only using Tensorlake.

### Acceptance criteria

1. **Hide unready, unrecorded places on multi-provider recipe cards (provider-neutral):**
   - In the helm's recipe-places projection (`crates/farhelm-helm/src/hosts.rs` `recipe_places` and the exhaustive
     recipe/base helpers mapped in `crates/farhelm-helm/src/environment/mod.rs`), include a candidate place on a recipe
     only when at least one of these holds:
     1. the place is **usable** (no readiness refusal `reason`), OR
     2. the recipe has **visible recorded state or an active job** at that place — add an exhaustive read-only
        `RecipeState::has_visible_state(place)` (covering built images, `last_failure`, and pending cleanup without
        altering `needs_cleanup()` / `recorded_places()`, which intentionally exclude `last_failure` so failed builds do
        not block recipe deletion) combined with `RecipeBuilds`'s in-memory active-job check, OR
     3. the recipe's base supports **only** this place's provider — add an exhaustive
        `RecipeBase::sole_provider(&self) -> Option<ProviderKind>` beside `RecipeBase::supports` (so `ContainerImage`
        still explains why Podman is unavailable rather than going blank).
   - Keep `EnvironmentProvider::places()` itself returning every place with its refusal `reason` so launch refusals
     (`tensorlake_add.rs`, `agent_requests.rs`), `launcher/view/placement.rs`, and `Settings → Containers` are
     unaffected.
   - Follow the provider-exhaustiveness rule in `AGENTS.md` and `crates/farhelm-helm/src/environment/mod.rs`: express
     any provider-specific visibility choice via exhaustive methods on `RecipeBase` and `RecipeState` (never `matches!`
     over providers or `provider == Podman` in shared code).
2. **Disable `Rebuild` when a shown place is unusable:**
   - Add `#[serde(default)] pub unavailable_reason: Option<String>` on `RecipePlaceView`
     (`crates/farhelm-proto/src/recipes.rs`), populated directly from `availability.reason` in `hosts.rs:recipe_places`
     (do not overload `RecipePlaceView.reason`, which already holds `failure.or(freshness)` on _usable_ hosts where
     `Rebuild` must stay enabled, and which currently shadows `availability.reason` when `view.reason.is_some()`).
   - In `Settings → Recipes` (`crates/gui/farhelm-gui/src/settings/recipes/places.rs`), disable `Rebuild` whenever
     `place.building || place.unavailable_reason.is_some()`, showing `unavailable_reason` on hover (with zero
     Podman-specific branching in `places.rs`).
3. **Preserve zero-config Podman discovery and update specs/tests:**
   - Keep `Settings → Recipes` probing container hosts so Linux machines with usable Podman still appear automatically
     once checked, while machines without Podman stay completely silent on `Managed Ubuntu` recipe cards.
   - Update `SPEC.md` (and `SPEC_impl.md` if applicable) where recipe-card per-place status is described.
   - Update and add helm and GUI tests (`crates/farhelm-helm/src/ui_channel_tests.rs`,
     `crates/gui/farhelm-gui/src/settings/recipe_place_tests.rs`, and unit tests covering usable vs. unusable,
     recorded-state retention, single-provider base retention, and disabled `Rebuild` with hover reason).
   - Remove the covered `TODO.md` entry in the final PR of the stack.

## Decisions already made

### Maintainer decisions (binding)

- **Visibility rule:** Show a container place on a recipe card only when the host is usable, OR the recipe already has
  recorded state / active build / error there, OR the recipe's base supports only that provider (`ContainerImage`).
- **`Rebuild` button on unusable shown rows:** Disable `Rebuild` while the place is unusable, with the reason on hover.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- `AGENTS.md` & `crates/farhelm-helm/src/environment/mod.rs`: Every disposable-environment provider decision belongs in
  its implementation or an exhaustive method on the provider kind, place, binding, recipe base, recipe state, host kind,
  or shared recipe projection. Shared code must not compare individual providers, use `matches!` over providers, or add
  a wildcard provider arm. UI wording, marks, and help belong in `crates/gui/farhelm-gui/src/environment_ui.rs`.
- `AGENTS.md`: Never mutate process environment variables in tests; never touch the live Farhelm install; annotate every
  test sleep with `// sleep-ok:` passing `scripts/check-test-sleeps.py`.

### Planner-chosen implementation shape

- **Filter in `hosts.rs:recipe_places` using exhaustive `RecipeBase::sole_provider()` and
  `RecipeState::has_visible_state(place)` + active-job check** rather than filtering only in the GUI or altering
  `needs_cleanup()` / `EnvironmentProvider::places()`. Filtering at the helm's recipe-places projection keeps
  `GET /api/recipe-places` and `Collection::RecipePlaces` consistent for every client, preserves recipe-deletion
  semantics (`needs_cleanup()` stays untouched), leaves `EnvironmentProvider::places()` untouched for launch refusals,
  and requires no new persisted state or Settings toggle.
- **Add `#[serde(default)] pub unavailable_reason: Option<String>` on `RecipePlaceView`** so
  `settings/recipes/places.rs` disables `Rebuild` and shows the hover reason cleanly without confusing usability with
  `view.reason` (which holds build failures and freshness on usable hosts) and without leaking Podman-specific host
  state into shared UI code.

## Outline

### Existing mechanisms reused

- `crates/farhelm-helm/src/hosts.rs` (`recipe_places` projection for `GET /api/recipe-places` and
  `Collection::RecipePlaces`).
- `crates/farhelm-helm/src/environment/mod.rs`, `crates/farhelm-helm/src/environment/podman.rs`, and
  `crates/farhelm-helm/src/environment/recipes.rs` (`EnvironmentPlace`, `RecipeBase::supports`, `recorded_places()`).
- `crates/farhelm-proto/src/recipes.rs` (`RecipePlaceView`, `RecipeBase`).
- `crates/gui/farhelm-gui/src/environment_ui.rs` (`ProviderUi::recipe_place`) and
  `crates/gui/farhelm-gui/src/settings/recipes/places.rs` (`Rebuild` button + hover help).

### PR stack

1. **`plan/hide-unconfigured-podman-recipe-status/01-filter-unusable-recipe-places-and-disable-rebuild`** — Filter
   unusable, unrecorded places out of multi-provider recipe cards at the helm's `recipe_places` projection (via
   exhaustive provider/base methods, preserving single-provider bases and recorded state), disable `Rebuild` with hover
   reason when an unusable place is shown in `Settings → Recipes`, update `SPEC.md` and helm/GUI tests, add the
   changelog fragment, and remove the covered `TODO.md` entry.

## Fallbacks

- If existing tests assert that an unprobed Podman host appears in `GET /api/recipe-places` for a `Managed Ubuntu`
  recipe, update those tests to reflect the new contract (and add explicit tests for all three keep conditions: usable,
  recorded state, and single-provider `ContainerImage` base).
- Scope-reassessment triggers (examples, not a blacklist): adding a new persisted "enable containers" column/setting,
  changing `Settings → Containers` behavior, or modifying `EnvironmentProvider::places()` in a way that drops launch
  refusal reasons.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-hide-unconfigured-podman-recipe-status-log.md` in the parent directory of the checkout you run in, derived
as that section says, and the watchdog's status file is
`farhelm-plan-hide-unconfigured-podman-recipe-status-watchdog.json` beside it. Resolve both to absolute paths before you
start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`, "Agent
scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- Targeted Rust tests (through `record-test-run.py` with pinned nextest on `PATH`):
  - `cargo nextest run -p farhelm-proto -p farhelm-helm --lib -E 'test(recipe_place) | test(ui_channel) | test(environment::)'`
  - `cargo nextest run -p farhelm-gui --lib -E 'test(recipe_place) | test(settings::recipes)'`
- Formatting, lints, and checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
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
- The last PR of the stack removes the `TODO.md` entry this plan covers ("Hide unrelated Podman status noise on
  Tensorlake recipe cards."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred items added to
  `TODO.md` go under `## Plan follow-ups`.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
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
alternatives considered: in particular how `RecipePlaceView` signals usability and how `hosts.rs:recipe_places` keeps
the visibility rule exhaustive across providers, and every review finding you decided not to follow. The maintainer will
ask for these later.

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

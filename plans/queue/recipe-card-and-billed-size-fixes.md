# Recipe card removed-host cleanup and Tensorlake billed-size consent

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Implement two recipe-related `TODO.md` entries in a clean 2-PR stack — one from `## Immediate` (PR 1, required) and one
from `## Plan follow-ups` (PR 2, subject to the complexity kick-back rule below). Verbatim:

1. (`## Immediate`)
   > **Hide removed container hosts with only a failed build record on recipe cards.** After #4934, a container host
   > that was removed after a failed build or launch preparation (with no images or resources left) stays on the recipe
   > card permanently as "Unavailable place" with a disabled Rebuild button, and nothing removes it except deleting the
   > recipe (even re-adding the host gives it a new identity). In `recipe_places` (`crates/farhelm-helm/src/hosts.rs`),
   > keep a place the provider no longer lists only while the recipe still has resources (`has_resources`) or active
   > build work there, while continuing to show failure-only state for hosts that are still registered.

2. (`## Plan follow-ups` — kick-back rule applies)
   > **Bind Tensorlake launch consent to the billed size shown.** A recipe edit under the same name can make a confirmed
   > new sandbox bill for a different CPU, memory or disk size. The launcher's `sandbox_question.rs` freezes the words,
   > then sends only the recipe, place and a confirmation flag. Carry the displayed `RecipeSize` from that actual answer
   > through `POST /api/environment-sessions`. Compare it against the same definition that `start_session_environment`
   > uses to register the host, before registration or provider calls. When the size differs, refuse with the launch
   > draft available so the user can review the new size. A check of the desktop catalog alone leaves a race before
   > admission. Specify this route's precondition in [Sandbox sessions](SPEC.md#sandbox-sessions) and SPEC_impl.md under
   > “Questions: the prompt”. Settle compatibility for older clients and the legacy sandbox route carrying only
   > `confirmed`. If an older helm refuses the new size field, keep the refusal and require a helm that can check it.
   > Preserve the deliberate skip setting and ordinary container launches. Test a same-name edit after the prompt, prove
   > no allocation on a mismatch, and review the refusal in both appearances. The protocol, validation and visual work
   > exceed this triage item's size gate.

### Acceptance criteria

1. **PR 1 — Hide removed failure-only container hosts on recipe cards (Immediate, always implemented):**
   - A container host removed from Farhelm that only has a stored build/prepare failure (no images, containers, retired
     images, or active build work) is omitted from `/api/recipe-places` for both shared (`ManagedUbuntu`) and
     single-provider (`ContainerImage`) recipes.
   - Removed container hosts that still have resources (`has_resources`) or active build work remain shown so cleanup
     stays visible; still-registered hosts (usable or unavailable) keep their existing visibility rules including
     failure-only state.
   - Remove dead `RecipeState::visible_places()` in `crates/farhelm-helm/src/environment/recipes.rs`, update tests in
     `crates/farhelm-helm/src/ui_channel_tests.rs`, and remove the Immediate `TODO.md` entry in PR 1.
2. **PR 2 — Bind Tensorlake launch consent to the billed size shown (Plan follow-up, kick-back rule applies):**
   - Carry the displayed `RecipeSize` from the answered confirmation in `sandbox_question.rs` through
     `POST /api/environment-sessions` (`confirmed_size: Option<RecipeSize>`).
   - On `POST /api/environment-sessions`, require `confirmed_size` when `confirmed: true` (for providers that ask before
     creating, or whenever `confirmed: true` is sent); on legacy `POST /api/sandbox-sessions`, keep `confirmed_size`
     optional (accept omitted, and validate when supplied). Whenever `confirmed_size` is supplied, compare it against
     `definition.size` in `start_session_environment` before registration or any provider calls.
   - On a size mismatch, refuse before any registration or provider call and return to the launcher with the launch
     draft available so the user can review the updated size. Preserve the deliberate skip-asking setting and ordinary
     container launches, and keep `#[serde(deny_unknown_fields)]` refusals from older helms intact.
   - Update `SPEC.md` ("Sandbox sessions") and `SPEC_impl.md` ("Questions: the prompt"), add tests for a same-name edit
     after the prompt proving no allocation on mismatch, review the refusal in light and dark appearances, and remove
     the follow-up `TODO.md` entry in PR 2 (unless kicked back under the complexity rule below).

### Complexity kick-back rule (applies to PR 2 only)

The maintainer's instruction for `Plan follow-ups` items: _"If this ends up being complicated, kick it back to the to-do
queue. The executor should not continue if this turns very complicated."_ PR 1 is an `Immediate` item and is always
implemented. For PR 2, if implementing it turns very complicated (sprawling beyond the reviewed outline into unforeseen
protocol/subsystem complexity), stop work on PR 2 instead of pushing through: leave its `TODO.md` entry in
`## Plan follow-ups` updated with concrete findings explaining what made it complicated, log a `DECISION`, keep PR 1
clean and delivered, and explain the kick-back in the delivery report.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped into one plan:** Approved by maintainer.
- **Older-client compatibility for `confirmed_size`:** Require `confirmed_size` when `confirmed: true` on
  `POST /api/environment-sessions`; keep it optional on legacy `POST /api/sandbox-sessions`.
- **Complexity kick-back:** Applies to the billed-size follow-up (PR 2), not the Immediate recipe-card fix (PR 1).
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md")
  and "Code rules" (per-provider decisions live in exhaustive provider methods, never comparing individual providers or
  using wildcard provider arms in shared code; read `crates/farhelm-helm/src/environment/mod.rs`).
- Follow `.agents/test-authoring.md`, `.agents/validation.md`, and `.agents/test-evidence.md`. Never mutate process
  environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`crates/farhelm-helm/src/hosts.rs`, `environment/recipes.rs`, `ui_channel_tests.rs`):**
   - In `recipe_places`, seed `candidates` from `build.recorded_places()` (already filtered by `has_resources`) plus
     provider-listed `places` plus `active_places`. Look up `listed = places.iter().find(|c| c.place == place)`. When
     `listed.is_some()`, apply the existing visibility check
     (`unavailable_reason.is_none() || view.building || build.has_visible_state(place) || recipe.base.sole_provider() == Some(place.provider())`);
     when `listed.is_none()` (removed host), keep only `view.building || build.has_resources(place)` so even
     `ContainerImage` recipes drop removed failure-only hosts.
   - Delete `RecipeState::visible_places()` (its only caller was `recipe_places`).
   - Update `recipe_places_keep_usable_history_and_single_provider_places` in `ui_channel_tests.rs` to assert removed
     failure-only hosts are hidden for both `ManagedUbuntu` and `ContainerImage`, while registered failure-only hosts,
     removed hosts with resources, and removed hosts with active build work stay shown.
2. **PR 2 (`farhelm-proto/src/session_requests.rs`, `farhelm-helm/src/sessions/sandbox.rs`,
   `farhelm-helm/src/provisioning/tensorlake_add.rs`, `farhelm-gui/src/launcher/view/sandbox_question.rs` &
   `view.rs`):**
   - Store `size: RecipeSize` on `SandboxAsk` in `sandbox_question.rs`, and replace `confirm_sandbox: bool` on
     `Launcher::submit` and `hold_for_creation_question` with `confirmed_size: Option<RecipeSize>` (avoids a duplicate
     bool+Option pair that could disagree). When `Some(size)`, set both `body.confirmed = true` and
     `body.confirmed_size = Some(size)`.
   - Add `#[serde(default)] pub confirmed_size: Option<RecipeSize>` to `EnvironmentCreateBody`,
     `EnvironmentCreateRequest`, and `SandboxCreateRequest` (keeping `#[serde(deny_unknown_fields)]`).
   - Share one internal helm admission helper between `create_environment_session` (`require_confirmed_size: true`) and
     `create_sandbox_session` (`require_confirmed_size: false`). Require `confirmed_size.is_some()` when
     `require_confirmed_size && confirmed && place.provider().asks_before_creating()` (reusing
     `asks_before_creating()`), and validate `confirmed_size == Some(definition.size)` whenever
     `confirmed_size.is_some()`.
   - Pass `confirmed_size: Option<RecipeSize>` into `ProvisioningService::start_session_environment` (`None` from
     `agent_requests.rs`) and compare against `definition.size` immediately after the existing `tensorlake_recipe_row`
     read — no new lock or second read needed. On mismatch, refuse cleanly so the launcher keeps the draft open with the
     refusal visible, and inspect the refusal visually in light and dark appearances.

## Scope reassessment

Before implementing a substantial departure from the outline (or if repeated corrective reviews hit the same component),
run a fresh-context review at your own model and effort with this charter verbatim:

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

Supply the request, decisions, outline, current diff, and proposed departure. A close reuse-versus-duplication call
within agreed scope is decided after reassessment and logged as `DECISION`. If a same-model sub-agent discovers an
unapproved new mechanism mid-unit, leave that part unimplemented and report it rather than inventing it.

## How to run

### Checkout check

This goal runs inside the checkout the executing session runs in, which must be a checkout of GitHub repository
`scode/farhelm` (the nearest `.git` or `.jj` checkout containing the working directory). Confirm that first. If it is
not, stop before the resume protocol, write nothing (not even the log), and report the mismatch. Every project path in
this file is relative to that checkout's root.

### Resume protocol

Right after confirming the checkout, invoke the `agent-resumeable` skill with the working log at
`farhelm-plan-recipe-card-and-billed-size-fixes-log.md` in the parent directory of the executing checkout. If the log
already exists, read it and resume where the previous session left off (cross-checking against VCS state and open PRs)
rather than starting over. If it does not exist, this is a fresh start.

### Execution mode

No-workhorse mode (explicit). Do not send any unit of your own decomposition to a weaker model or lower effort than your
own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point (if
`scode-galaxy-brain` is active, stop it). Same-model sub-agents at your own effort are allowed at your judgment when
they speed up independent work without losing quality (using separate worktrees or jj workspaces if concurrent); you
still own the `jjstack` stack and every PR. The sub-agents required by `plans/AGENTS.md`, "Sub-agents this file
requires" (resume check, cold reads of Blocked questions and reports), are exempt from any delegation restriction. Scope
reassessment runs at your own model and effort; the per-PR review gate keeps its demanded model and effort below.

### PR discipline

Build a linear stack of reviewable draft PRs using the `jjstack` skill, based on `main@origin` (or this plan's own open
PRs on resume, never another plan's PRs), per `plans/.agents/executing.md`, "Executing one plan". Use bookmarks
`plan/recipe-card-and-billed-size-fixes/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge.
Never edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on top.
Remove each covered `TODO.md` entry in the PR that completes it (or keep PR 2's entry updated if kicked back).

### Review gate

Before finishing any PR in this stack, run a fresh-context review of its changes using a **single fresh-context agent on
`gpt-6.2-sol` at `high` effort** (launched natively if available or through `scode-harness-shellout`, never hardcoding a
launch command here). Pass the executing checkout root, commit range or bookmark, and a private findings file in your
scratch directory, plus this full charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the supplied file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

When the PR changes tests or test fixtures, also supply `.agents/test-authoring.md` verbatim per `AGENTS.md`, "Tests and
validation". Address all valid findings before moving on.

### Resource watchdog

Right after the resume protocol and before launching sub-agents or builds, start the `scode-build-goal` skill's
`scripts/watchdog` (resolved via `realpath` next to that skill's `SKILL.md`), with status file
`farhelm-plan-recipe-card-and-billed-size-fixes-watchdog.json` beside the working log, watching the executing checkout
root, `$HOME`, `/tmp`, the log's parent directory, your scratch directory, and any worktrees on other filesystems, at a
60-second interval with default thresholds. Follow the `scode-build-goal` skill's Resource watchdog section for harness
wiring, startup verification, acting on alerts, and stopping the monitors at close.

### Decision logging and unattended fallback

Log major design or tradeoff choices in the working log as `DECISION` with alternatives considered.

This plan runs unattended. Never ask the maintainer anything in conversation — no direct questions, no harness question
tool, no optional follow-up offers, and no turn ending waiting for confirmation. Decide routine implementation forks
within the agreed scope and log them as `DECISION`. If a genuine fork requires a maintainer decision (material scope
expansion, weakened guarantee, omitted requirement, or unresolved design conflict), push all consistent work and block
per `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Things the maintainer should know and
possible follow-ups go into the delivery report.

### Done criterion

Every PR in the stack exists as a draft on GitHub, satisfies all acceptance criteria (with PR 1 implemented and PR 2
either implemented or cleanly kicked back to `TODO.md` under the complexity rule) and the latest `## Decisions` entry
(if any), and has passed its `gpt-6.2-sol` high review gate. Delivering the report is the executor's closing step (step
11 of `plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

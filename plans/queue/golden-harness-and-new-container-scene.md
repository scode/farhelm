# Golden harness outcome reporting and New container scene

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Implement two GUI golden-harness `TODO.md` entries from `## Plan follow-ups`, grouped as one 2-PR linear stack. Both
items are subject to the complexity kick-back rule below. Verbatim:

1. > **Show missing renders on the portable golden changes page.** A comparison can finish with both ordinary pixel
   > differences and scenes that never rendered. The page contains retained differences, so a reviewer cannot infer
   > coverage from missing images. Have the renderer report each selected light and dark appearance's render outcome
   > separately from its comparison result. Include missing or failed appearances in the page, including an update that
   > exits successfully with missing captures. A shared nonzero exit cannot distinguish these cases.

2. > **Show the settled build status in the New container golden.** The fake helm supplies the Development recipe but
   > never supplies its build state on the selected Container host. The scene stops waiting when the Recipe field is
   > drawn, which permits a picture of the interim status or an unavailable host. Stage a ready container host and a
   > build place for that exact host and recipe. Wait for the settled note to be drawn before comparing either
   > appearance. The reference images must move with the fixture. Refresh them on the current launcher and recipe-host
   > behavior, then inspect the full comparison for unrelated differences before publishing the new references.

### Acceptance criteria

Each implemented item removes its own `TODO.md` entry in its PR:

1. **PR 1 (Show missing renders on the portable golden changes page):**
   - The Rust golden renderer records each selected `(scene, appearance)` render outcome separately from pixel
     comparison diffs (distinguishing rendered-and-matched, rendered-with-diff, failed, and missing/never-rendered).
   - `scripts/golden_changes_page.py` renders missing or failed appearances prominently on the portable golden changes
     page alongside any pixel differences, including when there are zero PNG pixel differences.
   - `scripts/check-gui-goldens.py` fails compare mode (nonzero exit, keeping `run_dir`) and `--update` mode whenever
     expected appearances are missing or failed, even if worker processes exited `0`.
   - Unit tests cover mixed diffs + missing renders, zero-diff missing renders, failed render outcomes, and `--update`
     exiting `0` with missing captures.
2. **PR 2 (Show the settled build status in the New container golden):**
   - `LauncherScene::Container` stages a ready container host (`ready: true, building: false`) and a `RecipePlaces`
     record for that exact host and the Development recipe, waiting until the settled `"Ready on <host>"` note is drawn
     before comparing either light or dark appearance.
   - Only `LauncherScene::Container` changes (`HostMenu` and `ContainerCheckoutRefusal` stay untouched).
   - The `New container` golden reference images (light and dark) are refreshed on the current launcher and recipe-host
     behavior and verified against the full golden comparison for zero unrelated differences.

### Complexity kick-back rule (applies to every item in this plan)

The maintainer's instruction for `Plan follow-ups` items: _"If this ends up being complicated, kick it back to the to-do
queue. The executor should not continue if this turns very complicated."_ Evaluate each item independently. If either
item turns very complicated during execution, stop work on that item instead of pushing through: leave its `TODO.md`
entry in `## Plan follow-ups` updated with concrete findings explaining what made it complicated, log a `DECISION`,
continue with the other item in this plan, and summarize any kicked-back item in the delivery report.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped into one plan:** Approved by maintainer.
- **Settled build state in New container golden:** `"Ready on <host>"` (`ready: true, building: false`).
- **Complexity kick-back:** Applies to both items individually.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md").
- Follow `.agents/test-authoring.md`, `.agents/validation.md`, and `.agents/test-evidence.md`. Never mutate process
  environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`crates/gui/farhelm-gui/src/goldens/mod.rs`, `goldens/changes.rs`, `scripts/check-gui-goldens.py`,
   `scripts/golden_changes_page.py`, and their Python/Rust tests):**
   - Place the per-appearance outcomes directory inside `scripts/check-gui-goldens.py`'s existing
     `TemporaryDirectory(prefix=".baseline-", dir=tool.WORK)` (`Path(baseline) / "outcomes"`, passed via
     `FARHELM_GOLDEN_OUTCOMES_DIR`) rather than managing a second temp directory lifecycle.
   - Keep the outcome writer in `goldens/changes.rs` using the existing per-stem `<stem>.json` pattern so concurrent
     workers need no cross-process locking and mid-batch crashes still leave complete prior records.
   - Remove `golden_changes_page.py`'s early return on empty `captures/` when expected appearances are missing or
     failed, and make compare mode in `check-gui-goldens.py` treat missing/failed expected outcomes as failure (nonzero
     exit, keeping `run_dir`) even if workers exit `0`, matching `--update`'s `complete_update` strictness.
2. **PR 2 (`crates/gui/farhelm-gui/src/goldens/launcher.rs`, `goldens/mod.rs`, golden reference PNGs):**
   - Stage the ready `ContainerCheckView` (`ready: true, building: false`) and `Collection::RecipePlaces` entry (which
     `goldens/launcher.rs` already forwards via `extra`, matching `goldens/settings.rs`) scoped to
     `LauncherScene::Container` alone.
   - Avoid adding a 5th `session_list_scene_*` wrapper in `goldens/mod.rs`: let `extra` override `Collection::Hosts` in
     `session_list_scene_with_listing` or update the host row directly on the fake helm.
   - Wait on the settled `"Ready on <host>"` note before capture/comparison in both appearances, regenerate the
     `New container` references, and inspect the comparison for any unrelated diffs before finalizing.

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
`farhelm-plan-golden-harness-and-new-container-scene-log.md` in the parent directory of the executing checkout. If the
log already exists, read it and resume where the previous session left off (cross-checking against VCS state and open
PRs) rather than starting over. If it does not exist, this is a fresh start.

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
`plan/golden-harness-and-new-container-scene/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never
merge. Never edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on
top. Remove each completed item's `TODO.md` entry in its PR (or leave it updated if kicked back).

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
`farhelm-plan-golden-harness-and-new-container-scene-watchdog.json` beside the working log, watching the executing
checkout root, `$HOME`, `/tmp`, the log's parent directory, your scratch directory, and any worktrees on other
filesystems, at a 60-second interval with default thresholds. Follow the `scode-build-goal` skill's Resource watchdog
section for harness wiring, startup verification, acting on alerts, and stopping the monitors at close.

### Decision logging and unattended fallback

Log major design or tradeoff choices in the working log as `DECISION` with alternatives considered.

This plan runs unattended. Never ask the maintainer anything in conversation — no direct questions, no harness question
tool, no optional follow-up offers, and no turn ending waiting for confirmation. Decide routine implementation forks
within the agreed scope and log them as `DECISION`. If a genuine fork requires a maintainer decision (material scope
expansion, weakened guarantee, omitted requirement, or unresolved design conflict), push all consistent work and block
per `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Things the maintainer should know and
possible follow-ups go into the delivery report.

### Done criterion

Every non-kicked-back item's PR exists as a draft on GitHub, satisfies its acceptance criteria and the latest
`## Decisions` entry (if any), and has passed its `gpt-6.2-sol` high review gate (and any kicked-back item has its
updated `TODO.md` note and `DECISION` log entry). Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

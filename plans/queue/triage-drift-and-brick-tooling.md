# Live drift suite and release brick-test tooling fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

**Dependencies:** This plan runs after `agent-drift-and-screen-capture-tooling.md` and
`brick-test-reopen-and-retry-fixes.md` land on `main`, because they edit `scripts/run-harness-drift.py`,
`crates/farhelm/tests/e2e/real_agent_capture/drift/`, and `releasing/mac-vm-test/brick_test.py`.

## The goal

Execute nine settled maintainer-tooling `TRIAGE_OUTCOMES.md` entries covering the opt-in live agent drift suite and the
release brick-test coordinator, per `review_feedback_queue/.agents/execute-outcomes.md` with maintainer-approved grouped
PRs (one linear 3-PR stack):

1. `trailing-drift-copyback-runs-while-agent-may-live.md` (`fix code`)
2. `trailing-drift-panic-skips-copyback.md` (`fix code`)
3. `trailing-drift-failed-copyback-passes-case.md` (`fix code`)
4. `trailing-drift-runner-drops-requested-harness.md` (`fix code`)
5. `trailing-drift-cursor-initial-prompt-never-passes.md` (`fix code`)
6. `trailing-drift-capture-timing-check-too-coarse.md` (`fix code`)
7. `trailing-drift-claude-trust-real-config-byte-check.md` (`fix code`)
8. `trailing-drift-claude-trust-ancestor-vacuous-pass.md` (`fix code`)
9. `trailing-brick-cleanup-refuses-untested-remote-mac-pair.md` (`fix code`)

Each PR graduates its covered entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (with final
Execution record, change ID, bookmark, and PR URL) and removes their feedback files and `review_feedback_queue/INDEX.md`
lines in the same PR.

### Acceptance criteria

1. **PR 1 (`plan/triage-drift-and-brick-tooling/01-drift-copyback-and-runner`) —
   `trailing-drift-copyback-runs-while-agent-may-live.md`, `trailing-drift-panic-skips-copyback.md`,
   `trailing-drift-failed-copyback-passes-case.md`, and `trailing-drift-runner-drops-requested-harness.md`:**
   - In `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs` and `private_config.rs`, track confirmed session
     deletion and absence of failed-create leftovers in `DriftEnv` so `PrivateConfig::finish` gates copy-back
     accurately, emits notes when skipped, and corrects misleading comments in `private_config.rs`.
   - Catch panics around `run(...).await` in `with_drift_supervisor_using` so normal teardown and `config.finish(...)`
     run before resuming unwind.
   - Continue copying remaining files and running `real_leaves_unchanged()`, then fail the drift case (after printing
     notes) when any copy-back I/O replacement failed; update `docs/harness-drift-tests.md`.
   - In `scripts/run-harness-drift.py`, refuse with a clear error when any explicitly requested `--harness` has no
     matching case in the selected categories, and add coverage in `scripts/test-run-harness-drift.py`.
   - Add offline unit tests for all copy-back behaviors in `real_agent_capture::drift::private_config::tests` /
     `drift::tests`.
2. **PR 2 (`plan/triage-drift-and-brick-tooling/02-drift-cursor-timing-and-claude-trust`) —
   `trailing-drift-cursor-initial-prompt-never-passes.md`, `trailing-drift-capture-timing-check-too-coarse.md`,
   `trailing-drift-claude-trust-real-config-byte-check.md`, and
   `trailing-drift-claude-trust-ancestor-vacuous-pass.md`:**
   - In `crates/farhelm/tests/e2e/real_agent_capture/drift/initial_prompt.rs`, seed Cursor's case using
     `cursor-agent create-chat` (reusing the shared `vendor_lookup` helper from `drift/mod.rs`) so both fresh and named
     steps can pass. **Narrow** (do **not** delete whole) the matching `TODO.md` `## Plan follow-ups` entry ("Finish
     Cursor launch-prompt verification") so it retains only the remaining live-login verification once a Cursor login is
     available (do not run live vendor drift tests in this unattended plan).
   - In `drift/capture_timing_hooks.rs`, assert hook-log ordering (prompt/start capture line precedes `event=Stop`) for
     `FirstPrompt` and `FirstPromptWithHooks` agents, with offline unit tests.
   - In `drift/private_config.rs`, scope copied Claude real-config isolation checks so the case fails only if the real
     file gained `hasTrustDialogAccepted` for the scratch workspace key (emitting a `drift:` note on unrelated byte
     changes, updating `docs/harness-drift-tests.md`), and strip `hasTrustDialogAccepted` / ancestor `projects` entries
     for `work.ancestors()` from the private mirror copy before launch; add offline unit tests.
3. **PR 3 (`plan/triage-drift-and-brick-tooling/03-brick-cleanup-untested-remote-pair`) —
   `trailing-brick-cleanup-refuses-untested-remote-mac-pair.md`:**
   - In `releasing/mac-vm-test/brick_test.py`, require `native_preflight_retained(entry)` during `cleanup` only when the
     pass is in scope or already has results, preflight, or tester evidence (matching `BRICK-TEST.md:715-716`), and add
     a unit test in `releasing/mac-vm-test/test_native_coordinator.py`.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped plan and grouped PRs:** Approved by the maintainer on 2026-11-25.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "Flows" -> Real agent
  tests: opt-in only, never spend vendor turns in unattended runs), `review_feedback_queue/.agents/execute-outcomes.md`
  (with the maintainer-approved grouped PRs above), `.agents/validation.md` (`test-run-harness-drift.py`, offline
  `real_agent_capture::drift::` unit tests, portable brick-test unittest suite), `.agents/test-evidence.md`, and
  `.agents/test-authoring.md`.
- Never mutate process environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live
  Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`drift/mod.rs`, `drift/private_config.rs`, `scripts/run-harness-drift.py`,
   `scripts/test-run-harness-drift.py`, `docs/harness-drift-tests.md`):**
   - Reuse existing `PrivateConfig::finish`, `Mirror::copy_back`, and `AssertUnwindSafe(...).catch_unwind().await`.
2. **PR 2 (`drift/initial_prompt.rs`, `drift/capture_timing_hooks.rs`, `drift/private_config.rs`, `TODO.md`,
   `docs/harness-drift-tests.md`):**
   - Reuse `vendor_lookup` (moved to `drift/mod.rs` by `agent-catalog-cli-and-drift-cleanups`) and the `create-chat`
     pattern already in `drift/named_conversation.rs`. Narrow rather than delete the `TODO.md` Cursor entry so live
     login verification stays tracked.
3. **PR 3 (`releasing/mac-vm-test/brick_test.py`, `test_native_coordinator.py`):**
   - Narrow the existing `native_preflight_retained(entry)` check in `cleanup`; no new coordinator state.

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
`farhelm-plan-triage-drift-and-brick-tooling-log.md` in the parent directory of the executing checkout. If the log
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
`plan/triage-drift-and-brick-tooling/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge.
Never edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on top.
Graduate each covered outcome to `TRIAGE_OUTCOMES_ARCHIVE.md` and remove its feedback file and
`review_feedback_queue/INDEX.md` entry in the same PR.

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
`farhelm-plan-triage-drift-and-brick-tooling-watchdog.json` beside the working log, watching the executing checkout
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

Every PR in the stack exists as a draft on GitHub, satisfies its acceptance criteria and the latest `## Decisions` entry
(if any), graduates its covered `TRIAGE_OUTCOMES.md` entries and removes their queue files/index lines, and has passed
its `gpt-6.2-sol` high review gate. Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

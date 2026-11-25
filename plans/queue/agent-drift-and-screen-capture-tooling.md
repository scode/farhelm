# Agent drift and screen-capture tooling fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Implement three agent-drift and screen-capture tooling `TODO.md` entries from `## Plan follow-ups`, grouped as one 3-PR
linear stack. All three items are subject to the complexity kick-back rule below. Verbatim:

1. > **Stop the agent updater's process group before running drift cases.** `scripts/harness_upgrades.py` currently
   > times out the top-level process. An updater or fallback installer child can still replace the CLI after the runner
   > probes its version, so a case can report a verdict for the wrong binary. Cover both timeouts and cancellation
   > during preparation, including a signal arriving as the process starts. Reuse the same teardown for the updater and
   > the fallback shell, and verify with a local child process that no write can race the version probe or a case.

2. > **Publish captured agent screens as recoverable pairs.** `scripts/capture-agent-screens.py` copies each scrubbed
   > screen and title from staging into a version directory. An I/O failure or interruption can leave those two files
   > from different captures and remove the staged replacement. Preserve the previous matched pair, retain the validated
   > replacement on failure, and let a later run recover any interrupted publication. Renaming each file atomically
   > still leaves an interruption window between the two renames. Settle the pair publication and recovery contract with
   > the fixture loader, then exercise both an I/O failure and an interrupted replacement using stand-ins.

3. > **Require a committed hook report after Restart in the drift checks.** The Codex, Antigravity and Grok cases can
   > see a new acknowledged callback and the old stored resume identity even when the callback was a stale no-op.
   > Require evidence that the resumed generation committed the report. Carry the generation and identity in the witness
   > used by the drift tests, and cover an acknowledged same-identity no-op and a valid resumed report. The supervisor
   > emits `acked` for committed and ignored callbacks. The drift matcher needs a distinct commit witness from
   > supervisor admission or a new resume pin bound to the resumed generation.

### Acceptance criteria

Each implemented item removes its own `TODO.md` entry in its PR:

1. **PR 1 (Stop the agent updater's process group before running drift cases):**
   - `scripts/harness_upgrades.py` runs updaters, fallback installers, and `--version` probes in their own process group
     (`start_new_session=True`) and terminates/reaps the entire process group on timeout or cancellation (including
     `SIGINT`/`SIGTERM` arriving during startup or fallback download in `prepare()`).
   - Preparation never proceeds to version probing or drift cases while an updater or installer descendant is still
     alive, verified by local child-process tests in `scripts/test-run-harness-drift.py`.
2. **PR 2 (Publish captured agent screens as recoverable pairs):**
   - `scripts/capture-agent-screens.py` publishes `.txt`/`.title` pairs through a `.capture-publish/` rollback journal
     inside `<harness>/<version>/`, preserving the previous matched pair on failure, retaining the validated staged
     replacement (skipping `work_root` cleanup and printing its path), and recovering any interrupted
     `.capture-publish/` directory on the next run.
   - The Rust fixture loader (`crates/farhelm-supervisor/tests/screen_fixtures.rs`) fails loudly if `.capture-publish`
     or an orphan `.title` without a matching `.txt` is present in a version directory.
   - Stand-in tests exercise both an I/O failure mid-swap and recovery of an interrupted publication, plus the Rust
     loader refusal.
3. **PR 3 (Require a committed hook report after Restart in the drift checks):**
   - Hook admission returns `Committed { generation }` vs `Ignored` and records the distinction and `gen=<N>` in the
     hook log unambiguously.
   - All existing hook-log readers share a small parser helper in `hook_report.rs` and stay compatible (including
     `codex_identity.rs`, `hook_identity.rs`, `real_agent_capture.rs`, and `drift/`).
   - Codex, Antigravity, and Grok drift checks require a committed hook report for the resumed generation after Restart,
     with unit tests covering both an acknowledged same-identity no-op (rejected) and a valid resumed-generation
     committed report (accepted).

### Complexity kick-back rule (applies to every item in this plan)

The maintainer's instruction for `Plan follow-ups` items: _"If this ends up being complicated, kick it back to the to-do
queue. The executor should not continue if this turns very complicated."_ Evaluate each item independently. If any item
turns very complicated during execution, stop work on that item instead of pushing through: leave its `TODO.md` entry in
`## Plan follow-ups` updated with concrete findings explaining what made it complicated, log a `DECISION`, continue with
the remaining items in this plan, and summarize any kicked-back item in the delivery report.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped into one plan:** Approved by maintainer.
- **Hook commit witness:** Return `Committed` (with generation) vs `Ignored` from hook admission and record it in the
  hook log (`gen=<N>`).
- **Complexity kick-back:** Applies to all three items individually.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Frozen material": do not modify committed captured agent screen fixtures under
  `crates/farhelm-supervisor/tests/fixtures/screens/`).
- Follow `.agents/test-authoring.md`, `.agents/validation.md`, and `.agents/test-evidence.md`. Never mutate process
  environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live Farhelm install; never
  run live vendor drift tests or spend vendor turns.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`scripts/harness_upgrades.py`, `scripts/run-harness-drift.py`, `scripts/test-run-harness-drift.py`):**
   - Keep a small, self-contained process-group runner inside `scripts/harness_upgrades.py` rather than generalizing
     `scripts/record-test-run.py` (which is a 2,000-line forensic evidence recorder with a different contract).
   - `run_installer` already delegates to `run_updater`, so one runner covers both; reuse it in `version()` too (with
     `timeout=VERSION_TIMEOUT`) so a hung `--version` probe also cleans up its process group.
   - Install `SIGINT`/`SIGTERM` handling across the whole `prepare()` loop in `run-harness-drift.py` (including
     `run_installer`'s `urllib` download) so a signal aborts preparation immediately and runs `finally` teardown rather
     than letting `SIGTERM` bypass cleanup or proceed into drift cases.
2. **PR 2 (`scripts/capture-agent-screens.py`, `scripts/test-capture-agent-screens.py`,
   `crates/farhelm-supervisor/tests/screen_fixtures.rs`):**
   - Prefer **rollback-only** recovery in `<harness>/<version>/.capture-publish/` (single recovery path, simpler than
     dual roll-forward + roll-back): stage replacement pairs in `.capture-publish/new/`, back up overwritten prior pairs
     in `.capture-publish/old/`, write affected stems and backup presence to `.capture-publish/manifest.json`, then swap
     pairs into the version directory.
   - Both mid-swap I/O failure and next-run recovery of an interrupted `.capture-publish/` call the exact same recovery
     function: restore every backed-up pair from `old/`, remove any newly introduced files named in `manifest.json` that
     were not in `old/`, and delete `.capture-publish/`.
   - On publication failure, skip `capture-agent-screens.py`'s `finally` deletion of `work_root` (even without
     `--keep-work`) and print the retained staging path.
   - In `screen_fixtures.rs`, fail loudly when `.capture-publish` or an orphan `.title` without a matching `.txt` exists
     in a version directory.
3. **PR 3 (`crates/farhelm-supervisor/src/hook_report.rs`, `service/core.rs`, `vendor/omp.rs`,
   `crates/farhelm/tests/e2e/real_agent_capture.rs`, `drift/mod.rs`, `drift/capture_timing_hooks.rs`,
   `codex_identity.rs`, `hook_identity.rs`):**
   - Propagate `Committed { generation }` vs `Ignored` from hook admission (`vendor/omp.rs` and
     `report_conversation_legacy` have no silent ignore paths and are mechanical return-type propagation).
   - Watch `hook_report.rs`'s `render_log_line` positional layout
     (`<unix-seconds> <word> [<detail> ]<conversation-id> <source>` — `detail` precedes `<conversation-id>`): record
     `Committed` vs `Ignored` and `gen=<N>` cleanly, add a small shared parser helper in `hook_report.rs` so
     `real_agent_capture.rs`, `drift/mod.rs`, `codex_identity.rs`, and `hook_identity.rs` do not duplicate
     `split_whitespace()` parsing, and update `codex_identity.rs` (which currently matches `Some("acked" | "refused")`
     including no-op Stop reports) if `Ignored` logs a distinct word such as `ignored`.
   - Check whether `TurnExpectation::hook_witness` (`HookWitness::Fresh` vs `MatchingIdentity` in `drift/mod.rs`) can be
     simplified once the log directly distinguishes `Committed` + `gen=<N>` from `Ignored`.

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
`farhelm-plan-agent-drift-and-screen-capture-tooling-log.md` in the parent directory of the executing checkout. If the
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
`plan/agent-drift-and-screen-capture-tooling/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never
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
`farhelm-plan-agent-drift-and-screen-capture-tooling-watchdog.json` beside the working log, watching the executing
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

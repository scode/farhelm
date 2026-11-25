# Supervisor Stop and Restart cleanup fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Implement two supervisor terminal-host lifecycle `TODO.md` entries from `## Plan follow-ups`, grouped as one 2-PR linear
stack. Both items are subject to the complexity kick-back rule below. Verbatim:

1. > **Settle an unconfirmed agent start before Stop reports success.** A launch can stay pending with no terminal when
   > Farhelm cannot confirm its terminal host ended. Authorized Stop can record an exit and return success without
   > checking the recorded start. A first stage that still holds the launch spec can start the agent after that reply.
   > Use Restart's recorded-start settlement under the lifecycle claim and after both consent guards. Include pinned
   > agent children in cleanup and keep the launch evidence if the recorded scope or process sweep is unconfirmed.
   > Preserve the distinction between a launch that left no live agent and a Stop that found one. The latter needs the
   > user-stop annotation even if a sweep retry no longer sees the child, and an outcome write failure must report that
   > cleanup already ran. A direct early sweep missed both cases. The final fix needs a durable stop-intent and retry
   > path beyond the triage budget. Extend the retained-create fixture in `host_creates.rs` to drive authorized Stop
   > against an unanswering and a gone host, and add an answering host with a live child. Assert refusal preserves the
   > launch, a completed retry preserves its stop annotation, and a failed write reports the work already done.

2. > **Keep failed Restart cleanup retryable after session deletion.** If a replacement fails to start after its session
   > row has been deleted, the supervisor can fail to confirm the replacement's Linux scope is empty. Startup can retire
   > the launch record once its host is gone, leaving processes from the failed attempt with no cleanup retry. Preserve
   > the exact generation's obligation under SPEC_impl.md's “Approved startup retry”, using the missing-row check that
   > decides whether to restore the failed Restart's session. An apparently empty scope is insufficient while the
   > recorded host start can still launch into it. Startup also needs to avoid a session-wide process sweep if a
   > retained create reservation has restored the same session ID. The local fix grew beyond its triage budget once
   > review identified both races. Exercise a failed sweep with a confirmed-gone host, a replacement start that can
   > still open its scope, and a restored session that must stay running. Ordinary Delete waits on Restart's lifecycle
   > claim, so remove the row in the fixture while Restart is held. Verify the retry also works with an empty session
   > table. Start with `unwind_failed_host_relaunch`, the failed-relaunch republish decision, and
   > `retry_pending_scope_cleanups`.

### Acceptance criteria

Each implemented item removes its own `TODO.md` entry in its PR:

1. **PR 1 (Settle an unconfirmed agent start before Stop reports success):**
   - Authorized Stop settles recorded unconfirmed agent starts under the lifecycle claim and after both consent guards,
     reusing Restart's settlement path.
   - When an answering unconfirmed host has a live child, Stop stops the child and keeps the host running for its final
     screen, matching ordinary Stop, and preserves the user-stop annotation across sweep retries even if a retry no
     longer sees the child.
   - If the recorded start cannot be confirmed settled or the scope/process sweep is unconfirmed, Stop refuses while
     preserving the launch evidence and does not transition `Launching` to `Exited` prematurely.
   - If the outcome write fails after cleanup already ran, Stop reports that cleanup already ran
     (`StopFailure::UnrecordedOutcome`).
   - Extended `host_creates.rs` fixtures cover authorized Stop against an unanswering host, a gone host, and an
     answering host with a live child, asserting refusal preserves the launch, a completed retry preserves the stop
     annotation, and a failed outcome write reports the work already done.
2. **PR 2 (Keep failed Restart cleanup retryable after session deletion):**
   - When a replacement fails to start after its session row was deleted and scope cleanup cannot be confirmed empty,
     the exact `(session_id, generation)` obligation is recorded in `pending_scope_cleanups` (no new table or schema
     migration).
   - Startup `retry_pending_scope_cleanups` settles its own exact `<id>.<gen>.host` launch record before trusting an
     empty scope (including when the session table is empty), keeps the obligation and record pending while the
     starter/host is not confirmed gone, and removes the `.host` record only after both host absence and scope cleanup
     succeed.
   - `reclaim_orphaned_hosts` skips any `(session_id, generation)` in `pending_scope_cleanups` so concurrent startup
     tasks never race on or delete that `.host` record, while preserving `discover_orphans()`'s `stored.is_empty()`
     database-loss fence.
   - When `known_session_ids()` (including retained create reservations) contains `id`, startup runs a scope-only sweep
     instead of a session-wide env-marker sweep so a restored session stays running.
   - Tests cover a failed sweep with a confirmed-gone host, a replacement start that can still open its scope, a
     restored session staying running, and startup retry with an empty session table.

### Complexity kick-back rule (applies to every item in this plan)

The maintainer's instruction for `Plan follow-ups` items: _"If this ends up being complicated, kick it back to the to-do
queue. The executor should not continue if this turns very complicated."_ Evaluate each item independently. If either
item turns very complicated during execution, stop work on that item instead of pushing through: leave its `TODO.md`
entry in `## Plan follow-ups` updated with concrete findings explaining what made it complicated, log a `DECISION`,
continue with the other item in this plan, and summarize any kicked-back item in the delivery report.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped into one plan:** Approved by maintainer.
- **Stop on an answering unconfirmed host:** Stop the child and keep the host running for its final screen, matching
  ordinary Stop.
- **Complexity kick-back:** Applies to both items individually.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md").
- Follow `.agents/test-authoring.md`, `.agents/validation.md`, and `.agents/test-evidence.md`. Never mutate process
  environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`crates/farhelm-supervisor/src/service/handlers.rs`, `crates/farhelm/tests/e2e/host_creates.rs`):**
   - Reuse existing `settle_outstanding_launches`, `stop_live_agent`, `LastOutcome::StopRequested`,
     `StopFailure::UnrecordedOutcome`, and `retire_gone_launch_records` (no new store schema or separate state machine;
     `store.rs` already turns `Transition::ObservedExit` on `LastOutcome::StopRequested` into the user-stop annotated
     exit).
   - Keep the ordering sharp in `handle_stop_session`'s non-live `else` branch: currently
     `sup.record(..., classification)` runs before `reap_process_tree`. When an unconfirmed `Launching` session
     (`terminal: None`) settles with a gone host (`settled.roots` empty, `gone_current_records` non-empty), run the
     scope/process sweep **before** recording `ObservedExit` and keep `gone_current_records` until the sweep succeeds so
     a sweep failure never transitions `Launching -> Exited` before cleanup is confirmed.
2. **PR 2 (`crates/farhelm-supervisor/src/service/core.rs`, `service/core/host_restart.rs`,
   `service/core/host_reclaim.rs`, `service/sweep.rs`, e2e tests):**
   - Reuse schema v38's existing `pending_scope_cleanups` table by recording `(id, claim.generation)` from the
     failed-relaunch missing-row path (`unwind_failed_host_relaunch` / `tear_down_relaunch_deleted_meanwhile`).
   - Do **not** rely on `reclaim_orphaned_hosts` to settle `<id>.<gen>.host` and do **not** weaken
     `discover_orphans()`'s `stored.is_empty()` safety fence:
     1. Have `retry_pending_scope_cleanups` settle its own exact
        `LaunchArtifacts::for_launch(&self.state_dir, &id, generation).host()` record via
        `self.settle_launch_record(&record, &socket)` before trusting an empty scope, leaving both obligation and
        `.host` record pending if the starter/host is not confirmed gone, and removing `.host` only once both host
        absence and scope cleanup succeed.
     2. Have `reclaimable_records` in `host_reclaim.rs` skip any `(session_id, generation)` currently in
        `pending_scope_cleanups` so the two concurrent startup tasks in `tokio::join!` never race on that `.host`
        record.
   - Reuse `sweep.rs`'s existing `reap_process_tree_with_sweep` / `ScopeUnits.needs_sweep` scope-only seam when
     `self.store.known_session_ids()` (which already includes retained create reservations) contains `id`, rather than
     duplicating scope kill/confirm logic in `host_restart.rs`.

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
`farhelm-plan-supervisor-stop-and-restart-cleanup-fixes-log.md` in the parent directory of the executing checkout. If
the log already exists, read it and resume where the previous session left off (cross-checking against VCS state and
open PRs) rather than starting over. If it does not exist, this is a fresh start.

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
`plan/supervisor-stop-and-restart-cleanup-fixes/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never
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
`farhelm-plan-supervisor-stop-and-restart-cleanup-fixes-watchdog.json` beside the working log, watching the executing
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

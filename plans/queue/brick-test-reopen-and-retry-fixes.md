# Brick test reopen binding and unavailable-P0 retry fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Implement two release brick-test harness `TODO.md` entries from `## Plan follow-ups`, grouped as one 2-PR linear stack.
Both items are subject to the complexity kick-back rule below. Verbatim:

1. > **Keep the brick test's installer pair retryable after an unavailable P0 check.** `record()` settles derived P1 and
   > P2 skips from a recoverable P0 observation. `retry-unavailable` then reads those skips as later executed checks.
   > Keep later observations and independent skips, and let the supported recovery discard only skips derived from the
   > retried checkpoint. Record why the eligible attempt can be repeated and preserve its evidence. Cover older saved
   > manifests as well as new ones. A change to the retry predicate alone leaves derived skips permanently recorded.

2. > **Bind the brick test's reopen samples to an app relaunch.** The bonus checkpoint currently checks that the remote
   > process and its counter survived between samples. The samples can come from the same app lifetime. Require this
   > attempt's before sample, a witnessed quit and fresh app identity, then the after sample. Carry that ordering
   > through guest receipt validation and host import. The counter and version checks alone cannot establish the
   > transition.

### Acceptance criteria

Each implemented item removes its own `TODO.md` entry in its PR:

1. **PR 1 (Keep the brick test's installer pair retryable after an unavailable P0 check):**
   - `record()` tags skips derived from an ordering block or dead end with
     `'derived_from': {'pass_id': ..., 'check_id': ...}`.
   - `retry_unavailable()` refuses if any later checkpoint in `ordered` has a non-skipped executed result or an untagged
     `skipped` entry (missing `derived_from`), **allows and keeps** skips whose `derived_from` names a different
     checkpoint (so retrying P0 `local-supervisor-agent` works even when earlier P1 `old-sessions` failed and left
     derived P1 skips after it), and discards only skips derived from `(pass_id, check)`, recording the discarded skips
     and retry reason in the `checkpoint_retries` archive entry so prior evidence is preserved.
   - No legacy detail-string parser is added for old manifests (explicit maintainer decision below); unit tests verify
     that untagged legacy skips are safely refused rather than silently cleared, alongside tests for clean retries,
     coexisting independent skips, and later executed results.
2. **PR 2 (Bind the brick test's reopen samples to an app relaunch):**
   - The `reopen` checkpoint records a dedicated `reopen-transition.json` receipt binding this attempt's
     `continuity_reopen_before` sample (and running candidate `before` `desktop_pids`), a witnessed quit + managed
     `launch` producing disjoint `after` `desktop_pids`, and this attempt's `continuity_reopen_after` sample matching
     the exported counter receipts.
   - `reopen` step ordering is tracked in per-attempt state (`self.observations` / `self.completed`), not pass-wide
     `self.state['reopen_before']`, so a stale sample from an earlier attempt cannot satisfy a later attempt.
   - Both guest receipt finalization (`guest_tester.py`) and host import (`brick_test.py`'s `record()` and `verdict()`)
     enforce the transition via one shared pure validator.
   - Unit tests cover same-lifetime samples (rejected), cross-attempt stale `before` samples (rejected), mismatched
     counter/receipt bindings (rejected), and a valid witnessed relaunch transition (accepted).

### Complexity kick-back rule (applies to every item in this plan)

The maintainer's instruction for `Plan follow-ups` items: _"If this ends up being complicated, kick it back to the to-do
queue. The executor should not continue if this turns very complicated."_ Evaluate each item independently. If either
item turns very complicated during execution, stop work on that item instead of pushing through: leave its `TODO.md`
entry in `## Plan follow-ups` updated with concrete findings explaining what made it complicated, log a `DECISION`,
continue with the other item in this plan, and summarize any kicked-back item in the delivery report.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped into one plan:** Approved by maintainer.
- **Older saved manifests for PR 1:** Going forward only — clear only skips carrying the new `derived_from` tag, and do
  not add a legacy detail-string parser. (This explicitly overrides _"Cover older saved manifests as well as new ones"_
  in the original TODO entry to minimize complexity; untagged skips simply remain refused.)
- **Complexity kick-back:** Applies to both items individually.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Before editing anything under `releasing/`, read `releasing/AGENTS.md` in full (and
  `releasing/mac-vm-test/BRICK-TEST.md` where it governs the harness contract). Do not run live macOS brick tests or
  touch any VM; validate with the Python unit tests under `releasing/mac-vm-test/`.
- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md").
- Follow `.agents/test-authoring.md`, `.agents/validation.md`, and `.agents/test-evidence.md`. Never mutate process
  environment variables in tests; never touch the live Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`releasing/mac-vm-test/brick_test.py` and its unit tests):**
   - Have `order_block()` and `dead_end()` return `(reason, cause)` where `cause` is `(cause_pass_id, cause_check_id)`
     or `None` (no new wrapper classes), storing `'derived_from': {'pass_id': ..., 'check_id': ...}` on `skip()` entries
     when `cause` is present.
   - In `retry_unavailable()`: scan `ordered[ordered.index(check) + 1:]`. Refuse on any non-skipped executed result or
     any untagged `skipped` entry (missing `derived_from`). Do **not** refuse on independent skips whose `derived_from`
     names a different checkpoint — in `p0-installer-upgrade`, P1 `old-sessions` runs before P0 `local-supervisor-agent`
     while its derived P1 skips (`local-old-session`, `remote-old-session`, etc.) sit after `local-supervisor-agent`, so
     independent skips must be preserved rather than blocking P0 retries. Discard only skips matching
     `derived_from == {'pass_id': pass_id, 'check_id': check}`, archiving them in `checkpoint_retries` alongside the
     retried attempt and retry reason.
2. **PR 2 (`releasing/mac-vm-test/guest_operations.py`, `guest_tester.py`, `brick_test.py`, `checkpoint_contract.py`,
   and unit tests):**
   - Keep `reopen-transition.json` separate from `restart-transition.json` (which only binds `before`/`after`
     `desktop_pids` for updater-driven `update-relaunch`), rather than blurring the two contracts.
   - Share one pure validator (`require_reopen_transition(...)` in `guest_tester.py` or `checkpoint_contract.py`, both
     already imported by `brick_test.py`) across guest finalization (`guest_tester.py`) and host import
     (`brick_test.py`'s `record()` and `verdict()`), and share a tiny `require_new_desktop_pids(before, after, final)`
     helper between `restart-transition` and `reopen-transition`.
   - Move `reopen`'s `continuity_reopen_before` -> quit + managed `launch` -> `continuity_reopen_after` tracking out of
     pass-wide `self.state['reopen_before']` into per-attempt state (`self.observations` / `self.completed`), reusing
     `guest_operations.py`'s existing receipt/ordering pattern with no new persistence layer.

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
`farhelm-plan-brick-test-reopen-and-retry-fixes-log.md` in the parent directory of the executing checkout. If the log
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
`plan/brick-test-reopen-and-retry-fixes/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge.
Never edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on top.
Remove each completed item's `TODO.md` entry in its PR (or leave it updated if kicked back).

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
`farhelm-plan-brick-test-reopen-and-retry-fixes-watchdog.json` beside the working log, watching the executing checkout
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

Every non-kicked-back item's PR exists as a draft on GitHub, satisfies its acceptance criteria and the latest
`## Decisions` entry (if any), and has passed its `gpt-6.2-sol` high review gate (and any kicked-back item has its
updated `TODO.md` note and `DECISION` log entry). Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

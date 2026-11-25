# Host provisioning action fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Implement two host-provisioning `TODO.md` entries from `## Plan follow-ups`, grouped as one 2-PR linear stack. Both
items are subject to the complexity kick-back rule below. Verbatim:

1. > **Bind a confirmed Tailscale Uninstall to the verified peer.** Uninstall checks the machine again at confirmation,
   > then resolves the Tailscale destination separately for each removal step. Keep the peer address, verified SSH keys,
   > and connection policy for the entire run, including the confirmation's checks and every destructive step. Reuse the
   > route's prepared identity for batch commands and reject a changed peer. The current plan stores a destination and
   > access mode, while command lowering owns the prepared identity and the key file. Carrying a single identity across
   > the run needs changes on both sides, with a fixture that checks which peer receives each action.

2. > **Give explicit host actions priority over automatic SSH sign-in.** A manual Update or Uninstall can join an
   > automatic update's pending sign-in, and answering can admit the automatic run before the manual request retries.
   > Turning Tailscale on or off can also hit a route lock held by an unanswered automatic check. Keep an attached
   > manual request's question and successful answer, but fence the automatic continuation through the manual retry. A
   > route edit must retire an automatic-only check and observe release of its route guard. Define how automatic retries
   > become eligible again and preserve a question that a manual action owns. Cover both shared authentication and route
   > edits in tests with automatic updates still on. Cancelling the prompt alone cannot arbitrate the admission race
   > after the answer.

### Acceptance criteria

Each implemented item removes its own `TODO.md` entry in its PR:

1. **PR 1 (Bind a confirmed Tailscale Uninstall to the verified peer):**
   - At confirmation, Uninstall resolves and re-verifies the Tailscale peer once, and pins that exact peer address,
     verified SSH keys, and connection policy (`SshAccess`) across every confirmation check and every destructive
     removal step of that run.
   - Batch commands and interactive sign-in reuse the route's prepared identity without re-resolving Tailscale per step,
     and reject a changed peer.
   - A fixture verifies which peer receives every confirmation check and removal step and confirms a peer change is
     rejected.
2. **PR 2 (Give explicit host actions priority over automatic SSH sign-in):**
   - When a manual Update or Uninstall joins an automatic update's pending SSH sign-in, the question and its successful
     answer are preserved for the manual action while automatic continuation is fenced for a bounded handoff window (30
     seconds) or until the manual run starts and settles.
   - Turning Tailscale on or off retires any automatic-only SSH check for that host and waits (bounded) for its
     `ssh_route_lock` release before acquiring the write lock (returning `HostBusy` only if the lock wait times out),
     while preserving any sign-in question owned by a manual action.
   - Automatic retries become eligible again once the manual run settles or the handoff window expires, and a
     failed/cancelled shared sign-in still records `AttemptState::Attempted(incarnation)` so it does not automatically
     re-prompt on the same connection.
   - Tests cover both shared authentication handoff and Tailscale route edits with automatic updates enabled.

### Complexity kick-back rule (applies to every item in this plan)

The maintainer's instruction for `Plan follow-ups` items: _"If this ends up being complicated, kick it back to the to-do
queue. The executor should not continue if this turns very complicated."_ Evaluate each item independently. If either
item turns very complicated during execution, stop work on that item instead of pushing through: leave its `TODO.md`
entry in `## Plan follow-ups` updated with concrete findings explaining what made it complicated, log a `DECISION`,
continue with the other item in this plan, and summarize any kicked-back item in the delivery report.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped into one plan:** Approved by maintainer.
- **Tailscale Uninstall peer pinning:** Resolve and re-verify once at confirmation, then pin that prepared peer identity
  for every check and removal step of that run.
- **Automatic SSH sign-in handoff and re-eligibility:** Fence automatic continuation for a bounded handoff window (30
  seconds) or until the manual run settles, and bound route-edit lock waits.
- **Complexity kick-back:** Applies to both items individually.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md").
- Follow `.agents/test-authoring.md`, `.agents/validation.md`, and `.agents/test-evidence.md`. Never mutate process
  environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`crates/farhelm-helm/src/access.rs`, `ssh_sign_in.rs`, `provisioning/plan.rs`, `provisioning/backend.rs`,
   `provisioning/service.rs`):**
   - Keep `RemoteRoute::Ssh` pure route configuration (avoid churning 20+ callers across `transport.rs`, `manager.rs`,
     `ssh.rs`, `tensorlake/ssh.rs`, and `podman_probe.rs`). Add
     `RemoteRoute::command_with_access(..., prepared: Option<&SshAccess>, ...)` reusing existing `ssh_command_args`,
     with `RemoteRoute::command` delegating `None`.
   - In `ssh_sign_in.rs`, factor the post-`SshAccess::prepare` body of `SignIn::establish` into
     `establish_with_access(..., access: &SshAccess, ...)` so interactive sign-in and batch commands share the exact
     prepared handle without duplicating `SignIn::establish`.
   - Carry the optional non-serialized `SshAccess` only on `ProvisioningTarget` (matching `RemoteRoute::PodmanExec`'s
     `#[serde(skip)] podman: PathBuf` precedent; `ProvisioningTarget` already has a manual `Serialize` impl), avoiding
     churn across `ProvisioningBackend`'s ~20 trait methods and test mocks.
   - Keep `ProvisioningTarget`'s plan-comparison equality independent of the runtime `SshAccess` handle (or attach it
     after comparison) so `revalidate`'s `replanned != pending.plan` check does not fail merely because confirmation
     attached the prepared handle, and update `revalidate` / `revalidate_and_execute` in `service.rs` so `execute` runs
     with the pinned target/plan instead of discarding `replanned`.
2. **PR 2 (`crates/farhelm-helm/src/provisioning/setup_sign_in.rs`, `provisioning/automatic.rs`, `hosts.rs`):**
   - Add `cancel_automatic_host(&self, host)` on `SetupSignIn` reusing `attempt.release()` (alongside existing
     `cancel_automatic_checks` and `cancel_host`).
   - In `hosts.rs`, update `set_tailscale` to call `cancel_automatic_host(host)` first and wait (bounded) for
     `ssh_route_lock(host).write_owned()`, returning `HostBusy` if the bounded wait times out. Do **not** add
     `ssh_route_lock` or `HostBusy` to `retarget` (`retarget` uses `host_provision_lock` + `host_write_lock` under
     `HOST_REPLY_BOUND` and already re-reads `row.destination` under `ssh_route_lock` before SSH; optionally call
     `cancel_automatic_host(host)` in `retarget` to retire a stale unanswered automatic check without changing its
     lock/error contract).
   - Reuse `automatic.rs`'s existing `service.manager.events().bump()`-driven `scan()` loop and `memory.busy` gate
     instead of introducing a second arbitration task or channel:
     - Record a per-host 30-second manual handoff deadline when a `manual`-attached check settles `Ready`, scheduling
       one timer that bumps `events()` on expiry; clear the deadline when a manual Update/Uninstall starts/plans on that
       host (after which `memory.busy` holds the fence until `end_run` bumps `events()`).
     - When a `manual`-attached check settles `Ready`, have `automatic.rs`'s `resume` task yield without calling
       `plan_update`/`admit` (sending `false` on `attempted` so `PendingSignIn` is removed rather than turning into
       `Attempted`), and have `scan()` skip hosts with an active handoff fence.
     - Preserve `AttemptState::Attempted(incarnation)` on `Failed` settlement so a failed/cancelled shared sign-in does
       not trigger an automatic re-prompt on the same connection.

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
`farhelm-plan-host-provisioning-action-fixes-log.md` in the parent directory of the executing checkout. If the log
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
`plan/host-provisioning-action-fixes/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge.
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
`farhelm-plan-host-provisioning-action-fixes-watchdog.json` beside the working log, watching the executing checkout
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

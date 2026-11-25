# Agent request boundaries and host identity pinning

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Execute five settled `TRIAGE_OUTCOMES.md` entries covering agent-request security boundaries, host identity pinning,
audit-log escaping, and repository-suggestion isolation, per `review_feedback_queue/.agents/execute-outcomes.md` with
maintainer-approved grouped PRs (one linear 3-PR stack):

1. `agent-environment-launch-skips-place-host-incarnation-pin.md` (`fix code`, highest; subject to maintainer complexity
   kick-back below)
2. `trailing-agent-create-connection-binding-untested.md` (`fix code`)
3. `helm-agent-create-dispatch-logs-asking-session-unescaped.md` (`fix code`, clearly settled; ~100-line size gate)
4. `preapproved-agent-requests-skip-deleting-session-refusal.md` (`fix code`)
5. `trailing-sandbox-agent-plants-repository-suggestions.md` (`fix spec+code`, high)

Each PR updates its covered `TRIAGE_OUTCOMES.md` entries (graduating each executed entry to the end of
`TRIAGE_OUTCOMES_ARCHIVE.md` with its final Execution record, change ID, bookmark, and PR URL) and removes its feedback
files and `review_feedback_queue/INDEX.md` lines in the same PR.

### Acceptance criteria

1. **PR 1 (`plan/triage-security-and-agent-boundaries/01-agent-env-place-pin`) —
   `agent-environment-launch-skips-place-host-incarnation-pin.md`:**
   - Before showing the approval card in `launch_environment_for_agent` (`crates/farhelm-helm/src/agent_requests.rs`),
     capture a provider-neutral `PlacePin` for the chosen place (for `Place::PodmanHost(host)`: the recorded
     `PodmanPlaceIdentity` — refusing before the card if none exists — plus
     `manager.status(host).map(|s| s.incarnation)`; for `Place::TensorlakeAccount`: a no-op pin).
   - Right after Allow, recheck the pin and refuse if the container host's identity or supervisor connection changed
     while the card waited (matching `approve_launch`'s refusal).
   - Pass `Option<PlacePin>` through `start_session_environment` into `ProviderHandle::register_session` and
     `store.register_pending_session_container`, asserting inside that SQLite transaction that `podman_place_claims` for
     `place` still holds the pinned `(machine_id, uid)` before inserting the pending container row (after which existing
     `refuse_referenced_place`, `claim_podman_place_with_twin`, and `run_verified` enforce the pin through
     `ensure_prepared` and `create`).
   - Add tests proving a retargeted container host is refused both during the approval-card wait and between Allow and
     pending-container registration, leaving no container created on the replacement machine.
   - **Maintainer complexity kick-back rule:** Only implement this if it stays straightforward using existing
     mechanisms. If during execution you find it is much more complicated than reusing existing `PlacePin` /
     `register_pending_session_container` / `podman_place_claims` mechanisms, stop work on this item, add a clear entry
     in `TODO.md` under `## Plan follow-ups` explaining why it is complicated, graduate this outcome to
     `TRIAGE_OUTCOMES_ARCHIVE.md` as deferred to that `TODO.md` entry (removing its queue file and index line), log a
     `DECISION`, and continue with PRs 2 and 3.
2. **PR 2 (`plan/triage-security-and-agent-boundaries/02-agent-request-guards-and-logging`) —
   `trailing-agent-create-connection-binding-untested.md`,
   `helm-agent-create-dispatch-logs-asking-session-unescaped.md`, and
   `preapproved-agent-requests-skip-deleting-session-refusal.md`:**
   - Add a focused helm test exercising `create_for_agent` -> `dispatch_agent_create` so dropping
     `approval.target_incarnation` forwarding fails the test.
   - Wrap `asking_session` with `escape_for_log(asking_session).as_str()` in `dispatch_agent_create`
     (`crates/farhelm-helm/src/agent_requests.rs`), matching all sibling tracing sites and `SPEC_impl.md` "Session IDs
     in logs", with a focused regression test (respecting its ~100-line clearly-settled size gate).
   - Check `state.approvals.session_is_being_deleted(&asking_key)` before the preapproval fast paths in
     `ask_with_answer` (`crates/farhelm-helm/src/approvals.rs`), with a regression test covering preapproved
     commands/own-sessions/native-link requests while `deny_session` is held.
3. **PR 3 (`plan/triage-security-and-agent-boundaries/03-exclude-agent-checkouts-from-repo-history`) —
   `trailing-sandbox-agent-plants-repository-suggestions.md`:**
   - Update `SPEC.md` ("Managed checkouts") to state that helm-wide repository suggestions come only from user-initiated
     Managed checkout launches and exclude agent-origin checkouts on every host.
   - In `crates/farhelm-helm/src/sessions.rs` (`record_created_history`), gate only `github_repo` on
     `origin == CreateOrigin::User` (leaving ordinary-host non-repository create history untouched, while also keeping
     `has_repository` false on session-owned hosts so `HistoryScope::RepositoryOnly` is skipped).
   - Add regression tests proving neither sandbox/container agent checkouts nor ordinary-host agent checkouts populate
     global repository suggestions, while user-initiated launches still do.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped plan and grouped PRs:** Approved by the maintainer on 2026-11-25.
- **`agent-environment-launch-skips-place-host-incarnation-pin.md`:** _"if this is easy using existing mechanisms, which
  it sounds like from the estimate. if the executor or planned realizes this iis much more complicated, kick it back to
  the todo list with an explanation of why its complicated."_
- **`trailing-sandbox-agent-plants-repository-suggestions.md`:** User-initiated launches only; exclude all agent-origin
  checkouts across every host (and update `SPEC.md`).
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "Code rules" — especially
  the exhaustive provider boundary in `crates/farhelm-helm/src/environment/mod.rs`: shared code must not compare
  individual providers or leak Podman-specific types into `EnvironmentProvider`).
- Follow `review_feedback_queue/.agents/execute-outcomes.md` (with the maintainer-approved grouped PRs above),
  `.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`.
- Never mutate process environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live
  Farhelm install; any deferred `TODO.md` entry goes in `## Plan follow-ups` (`plans/AGENTS.md`, "Follow-up TODO
  entries").

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`crates/farhelm-helm/src/environment/mod.rs`, `agent_requests.rs`, `provisioning/tensorlake_add.rs`,
   `store/podman.rs`):**
   - Do **not** thread `Option<PodmanPlaceIdentity>` through `EnvironmentProvider::ensure_prepared` or `Podman::create`
     (which would leak a Podman type across 8 trait implementations and blur shared recipe builds).
   - Instead, define a small provider-neutral `PlacePin` behind `environment/mod.rs`, check it before and after
     `approved(...)`, and enforce it atomically inside `store.register_pending_session_container`'s existing SQLite
     transaction (`store/podman.rs`). Once the pending container row references `place_host`, the existing
     `refuse_referenced_place`, `claim_podman_place_with_twin` (`PlaceClaim::Changed`), and `run_verified`
     (`VERIFY_PLACE_AND_EXEC`) chain already enforces the pin through `ensure_prepared` and `create`.
2. **PR 2 (`crates/farhelm-helm/src/agent_requests.rs`, `approvals.rs`):**
   - Reuse `escape_for_log` and `Approvals::session_is_being_deleted` directly; add a small test seam if needed only for
     exercising `create_for_agent` across `check_agent_approved`.
3. **PR 3 (`crates/farhelm-helm/src/sessions.rs`, `SPEC.md`):**
   - Gate only `github_repo` on `origin == CreateOrigin::User` in `record_created_history` rather than short-circuiting
     the entire function, preserving ordinary-host non-repo launch history.

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
`farhelm-plan-triage-security-and-agent-boundaries-log.md` in the parent directory of the executing checkout. If the log
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
`plan/triage-security-and-agent-boundaries/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never
merge. Never edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on
top. Graduate each covered outcome to `TRIAGE_OUTCOMES_ARCHIVE.md` and remove its feedback file and
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
`farhelm-plan-triage-security-and-agent-boundaries-watchdog.json` beside the working log, watching the executing
checkout root, `$HOME`, `/tmp`, the log's parent directory, your scratch directory, and any worktrees on other
filesystems, at a 60-second interval with default thresholds. Follow the `scode-build-goal` skill's Resource watchdog
section for harness wiring, startup verification, acting on alerts, and stopping the monitors at close.

### Decision logging and unattended fallback

Log major design or tradeoff choices in the working log as `DECISION` with alternatives considered.

This plan runs unattended. Never ask the maintainer anything in conversation — no direct questions, no harness question
tool, no optional follow-up offers, and no turn ending waiting for confirmation. Decide routine implementation forks
within the agreed scope and log them as `DECISION`. If a genuine fork requires a maintainer decision (material scope
expansion, weakened guarantee, omitted requirement, clearly-settled size gate exceeded without a TODO kick-back rule, or
unresolved design conflict), push all consistent work and block per `plans/.agents/executing.md`, "Executing one plan"
(step 10, Blocking). Things the maintainer should know and possible follow-ups go into the delivery report.

### Done criterion

Every PR in the stack exists as a draft on GitHub, satisfies its acceptance criteria and the latest `## Decisions` entry
(if any), graduates its covered `TRIAGE_OUTCOMES.md` entries and removes their queue files/index lines, and has passed
its `gpt-6.2-sol` high review gate (with any kicked-back PR 1 item documented in `TODO.md` `## Plan follow-ups` and
logged as `DECISION`). Delivering the report is the executor's closing step (step 11 of `plans/.agents/executing.md`),
not part of the goal.

## Release note

No release note needed.

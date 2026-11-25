# Drop the post-clone Claude workspace-trust edit

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Implement one `TODO.md` entry from the `## Immediate` bucket. Verbatim:

> **Drop the post-clone Claude workspace-trust edit.** Update `SPEC.md` and the implementation so workspace trust for
> agents whose config files must be edited (rather than supporting per-invocation trust flags) happens on launch of that
> agent kind, not earlier at checkout preparation. Stop editing `~/.claude.json` during managed-checkout creation when a
> session starts another agent (or before launch), along with the checkout-preparation Claude trust snapshotting and
> docs/tests that existed for that path.

### Acceptance criteria

1. **Spec rule updated:**
   - Update `SPEC.md` (and `SPEC_impl.md`) to state that workspace trust for agents whose config files must be edited
     (rather than supporting per-invocation trust flags) happens on launch of that agent kind, not earlier during
     managed-checkout preparation.
   - Update Claude's trust specification and user docs (`website/src/content/docs/docs/agents/claude.md`,
     `start-a-session.mdx`, etc.) so they no longer claim a fresh managed checkout started with another agent edits
     `~/.claude.json`, while still documenting best-effort archive-time trust removal for fresh managed checkouts
     launched with Claude.
2. **No post-clone Claude trust edit or cross-agent checkout-trust snapshotting:**
   - Remove the post-clone `if preparation.claude_trust` block from `prepare_github_checkout` in
     `crates/farhelm-supervisor/src/launch.rs`, and delete `CheckoutPreparation::claude_trust`.
   - Remove `ApprovedLaunchTrust` from `crates/farhelm-helm/src/workspace_trust.rs` and its callers in `sessions.rs`,
     `agent_requests.rs`, and `agent_launch.rs`, using `TrustResolvedLaunch` directly.
   - Stop setting `claude_trust: true` on `ResolvedGithubCheckout` (keep `ResolvedGithubCheckout::claude_trust` as
     `#[serde(default, skip_serializing_if = "std::ops::Not::not")]` ignored by the supervisor if needed for pre-upgrade
     retained create fingerprint compatibility, or remove cleanly and log as `DECISION`).
3. **Preserve best-effort archive-time removal for Claude-launched fresh managed checkouts:**
   - In `exec_launch_spec_with_seam` (`crates/farhelm-supervisor/src/launch.rs`), when `spec.claude_workspace_trust` is
     true (strictly structured Claude launches) and `spec.preparation` is `Some(prep)`, persist
     `record.claude_trust = Some(trust_record.clone())` into `prep.state_path` **while holding
     `acquire_preparation_lock(&preparation_lock_path(&prep.state_path))`** before writing `~/.claude.json`, and drop
     the lock before `exec()`. Existing `teardown.rs` archive removal continues to remove that entry when the checkout
     is archived or retired as missing.
4. **Hint band and Settings copy updated:**
   - In `crates/gui/farhelm-gui/src/settings/agent_trust.rs`, restrict Claude's `TrustAgent::asking` check to structured
     Claude launches (`agent == Some(Self::Claude)`), dropping `info.github_repo.is_some() ||`, and update Claude's
     Settings copy/risk text (plus goldens if any Settings scene text changes).
   - Update and add tests across helm, supervisor, and GUI proving:
     - a fresh managed checkout launched with a non-Claude agent never edits `~/.claude.json` even when Claude's trust
       switch is on;
     - a fresh managed checkout launched with Claude still trusts the checkout at launch time, records
       `PreparationRecord.claude_trust` under the preparation lock, and removes trust on archive;
     - Claude's trust hint band no longer appears for non-Claude sessions in fresh managed checkouts.
   - Remove the covered `TODO.md` entry in the final code PR.

## Decisions already made

### Maintainer decisions (binding)

- **General spec principle:** _"SPEC.md change - agent trusts, for agents that must have their configs changed rather
  than supporting per-invocation trust changes, happen on agent launch (of that agent kind), not earlier."_
- **Archive-time removal preserved for Claude-launched checkouts:** _"Record archive-time removal when a fresh managed
  checkout is launched with Claude"_ — _"recording the archive time removal is simply saying that we are scheduling the
  removal so that if everything goes well and nothing fails, we are removing the trust when we archive the managed
  checkout later. And so my selection here means that I do think that that should continue. Otherwise, we will just
  churn, churn, churn, and potentially add an unreasonable number of trust items in the user's configuration."_
- **Hint band scope:** Show Claude's workspace-trust hint band on structured Claude launches only (matching Codex, Muse,
  and Antigravity).
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Code rules": read `crates/farhelm-supervisor/src/agent_kind/mod.rs` before changing per-harness behavior).
- `crates/farhelm-supervisor/src/launch.rs` requires every read-modify-write of `prep.state_path` to hold the
  per-checkout flock (`acquire_preparation_lock`) against concurrent retries.
- Follow `.agents/test-authoring.md`, `.agents/validation.md`, and `.agents/test-evidence.md`. Never mutate process
  environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **Supervisor launch (`crates/farhelm-supervisor/src/launch.rs` & `service/core.rs`):**
   - Delete `CheckoutPreparation::claude_trust` outright (supervisor-internal, no `deny_unknown_fields`, older on-disk
     specs deserialize cleanly).
   - Remove the post-clone `if preparation.claude_trust` edit block in `prepare_github_checkout`.
   - In `exec_launch_spec_with_seam`, keep the single `if spec.claude_workspace_trust` call to
     `apply_claude_workspace_trust(Path::new("."), ...)`. When `spec.preparation` is `Some(prep)`, pass a closure that
     acquires `acquire_preparation_lock(&preparation_lock_path(&prep.state_path))`, reads `prep.state_path`, sets
     `record.claude_trust = Some(trust_record.clone())`, writes durably via `write_durable_sync` (preserving all other
     fields), and drops the lock when the closure returns (before `exec()`). `teardown.rs` needs no change.
2. **Helm (`crates/farhelm-helm/src/workspace_trust.rs`, `sessions.rs`, `agent_requests.rs`, `agent_launch.rs`):**
   - Remove `ApprovedLaunchTrust` and thread `TrustResolvedLaunch` directly through agent checkout and environment
     launch paths.
   - Stop setting `claude_trust` when constructing `ResolvedGithubCheckout` (the proto field already has
     `#[serde(default, skip_serializing_if = "std::ops::Not::not")]`, so keeping it as an ignored deprecated field or
     removing it is a close judgment call; log which way you choose as `DECISION`).
3. **GUI, specs, and docs (`crates/gui/farhelm-gui/src/settings/agent_trust.rs`, `SPEC.md`, `SPEC_impl.md`,
   `website/`):**
   - Drop `info.github_repo.is_some() ||` from `TrustAgent::asking` for Claude.
   - Update Claude's trust explanation in `agent_trust.rs`, re-capture/publish affected Settings goldens if wording
     changes rendered pixels (run visual review in both appearances), update `SPEC.md`, `SPEC_impl.md`, and affected
     docs pages (`bun install --frozen-lockfile && bun run build` in `website/`).

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
`farhelm-plan-drop-post-clone-claude-trust-log.md` in the parent directory of the executing checkout. If the log already
exists, read it and resume where the previous session left off (cross-checking against VCS state and open PRs) rather
than starting over. If it does not exist, this is a fresh start.

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
`plan/drop-post-clone-claude-trust/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge. Never
edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on top. Remove the
covered `TODO.md` entry in the final code PR.

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
`farhelm-plan-drop-post-clone-claude-trust-watchdog.json` beside the working log, watching the executing checkout root,
`$HOME`, `/tmp`, the log's parent directory, your scratch directory, and any worktrees on other filesystems, at a
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

Every PR in the stack exists as a draft on GitHub, satisfies all acceptance criteria and the latest `## Decisions` entry
(if any), has passed its `gpt-6.2-sol` high review gate, and removes the covered `TODO.md` entry. Delivering the report
is the executor's closing step (step 11 of `plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

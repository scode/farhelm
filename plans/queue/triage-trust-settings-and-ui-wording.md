# Workspace trust, Settings wording, and switch accessibility fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

**Dependency:** This plan runs after `triage-settings-nov24.md` lands on `main`, because both edit
`crates/gui/farhelm-gui/src/settings/agent_trust.rs`, `settings/hook_settings.rs`, and Settings goldens.

## The goal

Execute seven settled `TRIAGE_OUTCOMES.md` entries covering automatic workspace trust, Settings switch accessibility,
Settings trust help, disposable trust hints, and Restart approval-card trust wording, per
`review_feedback_queue/.agents/execute-outcomes.md` with maintainer-approved grouped PRs (one linear 3-PR stack):

1. `trailing-auto-trust-overrides-custom-flag-refusal.md` (`fix spec+code`, high — best-effort detection ONLY)
2. `trailing-unread-hook-and-spare-switches-read-as-off.md` (`fix code`)
3. `trailing-settings-trust-help-misdescribes-persistent-hosts.md` (`fix spec+code`)
4. `trailing-switch-warning-description-untested.md` (`other`, Nov 21 rule)
5. `trailing-disposable-trust-hint-says-off-by-default.md` (`fix spec+code`)
6. `trailing-stored-trust-chip-credits-settings-switch.md` (`fix spec+code`)
7. `trailing-trust-hint-late-refusal-untested.md` (`other`, Nov 21 rule)

Each PR graduates its covered entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (with final
Execution record, change ID, bookmark, and PR URL) and removes their feedback files and `review_feedback_queue/INDEX.md`
lines in the same PR.

### Acceptance criteria

1. **PR 1 (`plan/triage-trust-settings-and-ui-wording/01-auto-trust-custom-flags-best-effort`) —
   `trailing-auto-trust-overrides-custom-flag-refusal.md`:**
   - Extend `workspace_trust_blocked_by_selection` in `crates/farhelm-helm/src/launches.rs` (renaming/reusing
     `antigravity_custom_words` as `custom_permission_words`) to also detect Pi `--no-approve`/`--approve` (before `--`)
     and Codex project-trust `-c`/`--config` overrides (before `--`) on a small, best-effort basis without adding a TOML
     or general CLI flag parser.
   - Update `SPEC.md` (and `SPEC_impl.md` where applicable) to specify Custom trust-flag detection explicitly as
     **best-effort detection ONLY** — never documented or promised as a guarantee.
   - Add focused unit tests in `crates/farhelm-helm/src/launches.rs` and `workspace_trust.rs`.
2. **PR 2 (`plan/triage-trust-settings-and-ui-wording/02-settings-trust-wording-and-switch-a11y`) —
   `trailing-unread-hook-and-spare-switches-read-as-off.md`,
   `trailing-settings-trust-help-misdescribes-persistent-hosts.md`, and
   `trailing-switch-warning-description-untested.md`:**
   - Switch the hook-setup (`settings/hook_settings.rs`) and spare-checkout (`settings/checkouts.rs`) Settings toggles
     from `.disabled(...)` to `.value_unknown(...)` (matching `agent_trust.rs`), with tests verifying screen-reader
     state while unread/unreadable.
   - Update the workspace-trust Settings help copy in `crates/gui/farhelm-gui/src/settings/agent_trust.rs` and `SPEC.md`
     (lines describing installed vs persistent recipe hosts) so they align with `SPEC.md`'s rule that only a session's
     own disposable sandbox or container trusts automatically while local, SSH, and persistent hosts created with Add
     host follow Settings; update affected tests and goldens per `.agents/validation.md`.
   - Apply the Nov 21 rule to `trailing-switch-warning-description-untested.md`: add `node.description` assertions on
     the trust and hook switches if about 20 lines or fewer; otherwise leave untouched and record which way it went.
3. **PR 3 (`plan/triage-trust-settings-and-ui-wording/03-trust-hint-and-restart-chip-wording`) —
   `trailing-disposable-trust-hint-says-off-by-default.md`, `trailing-stored-trust-chip-credits-settings-switch.md`, and
   `trailing-trust-hint-late-refusal-untested.md`:**
   - Suppress the workspace-trust hint band in `crates/gui/farhelm-gui/src/session_pane/trust_band.rs` when the
     session's host has `HostRow::is_session_owned == true`, update `SPEC.md` to state that the hint applies only where
     the Settings switch applies, and add a regression test.
   - Give agent-requested Restart approval cards stored-launch trust-chip wording (e.g.,
     `"Saved with this session when it started. Repository hooks, settings and commands can run without asking."`),
     update `SPEC.md` to match, and update affected tests (and goldens if affected, per `.agents/validation.md`).
   - Apply the Nov 21 rule to `trailing-trust-hint-late-refusal-untested.md`: add the dismissal-then-refusal assertion
     if about 20 lines or fewer; otherwise leave untouched and record which way it went.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped plan and grouped PRs:** Approved by the maintainer on 2026-11-25.
- **`trailing-auto-trust-overrides-custom-flag-refusal.md`:** _"But SPEC this as best effort detection ONLY. We do NOT
  guarantee it and we never document it as a guarantee."_
- **Nov 21 rule on test gaps:** Fix only if about 20 lines or fewer and guarding user-facing behavior; otherwise drop
  with no change and record which way it went.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "Code rules": read
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` before changing per-harness behavior; no `kind == X` / `matches!`
  over specific harnesses in shared code).
- Follow `review_feedback_queue/.agents/execute-outcomes.md` (with the maintainer-approved grouped PRs above),
  `.agents/validation.md` (including GUI golden updates where drawn copy changes), `.agents/test-evidence.md`, and
  `.agents/test-authoring.md`.
- Never mutate process environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live
  Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`crates/farhelm-helm/src/launches.rs`, `workspace_trust.rs`, `SPEC.md`, `SPEC_impl.md`):**
   - `workspace_trust_blocked_by_selection` lives in `launches.rs`, called by `workspace_trust.rs`. Rename/reuse
     `antigravity_custom_words` as `custom_permission_words` across the exhaustive `LaunchHarness` arms (`Antigravity`,
     `Pi`, `Codex`) without introducing any TOML or general flag parser.
2. **PR 2 (`settings/hook_settings.rs`, `settings/checkouts.rs`, `settings/agent_trust.rs`, `settings/tests.rs`,
   `SPEC.md`):**
   - Reuse existing `Checkbox::value_unknown` directly, matching `agent_trust.rs`.
3. **PR 3 (`session_pane/trust_band.rs`, `approvals/model.rs`, `approvals/card.rs`, `SPEC.md`):**
   - Check `HostRow::is_session_owned` directly in `trust_band.rs`; keep Restart card wording distinct from Create cards
     in `approvals/model.rs` (note Prompt cards do not show the trust chip).

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
`farhelm-plan-triage-trust-settings-and-ui-wording-log.md` in the parent directory of the executing checkout. If the log
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
`plan/triage-trust-settings-and-ui-wording/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never
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
`farhelm-plan-triage-trust-settings-and-ui-wording-watchdog.json` beside the working log, watching the executing
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

Every PR in the stack exists as a draft on GitHub, satisfies its acceptance criteria and the latest `## Decisions` entry
(if any), graduates its covered `TRIAGE_OUTCOMES.md` entries and removes their queue files/index lines, and has passed
its `gpt-6.2-sol` high review gate. Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

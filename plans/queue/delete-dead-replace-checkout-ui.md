# Delete dead fresh-checkout Replace UI

## Goal

Remove the remaining unreachable fresh-checkout Replace wording and dead Replace checkout code in the desktop app
(following #5081 and #5142), and close the matching review-feedback queue entry
(`review_feedback_queue/trailing-existing-environment-checkout-note-dead.md`).
(`releasing/changelog.d/replace-checkout-lifetime.md` was already consumed by `v0.46.0`.)

Covers and removes this `TODO.md` entry in the last code PR:

- Immediate: **Delete dead fresh-checkout Replace code in the desktop app.**

### Acceptance criteria

- `EXISTING_ENVIRONMENT_CHECKOUT_LIFETIME` and the unreachable session-owned-host branch in `checkout_lifetime_note`
  (`crates/gui/farhelm-gui/src/launcher/view/checkout.rs`) are removed, along with its now-unused `cx: &App` parameter
  (updating its call site in `crates/gui/farhelm-gui/src/launcher/view.rs`).
- Dead Replace checkout match arms in `crates/gui/farhelm-gui/src/launcher/draft.rs` (`checkout_title`),
  `crates/gui/farhelm-gui/src/launcher/launch.rs` (`Request::checkout_path`), and
  `crates/gui/farhelm-gui/src/launcher/view/checkout.rs` (`replacement_folder_hint`, keeping its live behavior intact)
  are cleaned up.
- Live code that serves valid paths stays intact: `checkout_wake_notice` for non-session-owned sandbox hosts,
  `ReplaceWith` host-wake for a held folder, `replacement_folder_hint` / `replacement_checkout_source` explaining that
  Replace keeps the existing folder, and `Armed.source` / `submit_or_wait_for_preview`.
- `review_feedback_queue/trailing-existing-environment-checkout-note-dead.md` is deleted and its row removed from
  `review_feedback_queue/INDEX.md` per `review_feedback_queue/AGENTS.md`.
- All existing launcher tests continue to pass, and no dead-code warnings remain.

## Decisions already made

- **Cleanup scope (maintainer decision):** Full cleanup — remove all dead Replace-checkout remnants
  (`EXISTING_ENVIRONMENT_CHECKOUT_LIFETIME` + branch, plus the dead match arms in `checkout_title`,
  `Request::checkout_path`, and `replacement_folder_hint`), and close the matching review-feedback queue entry.
- **Review gate (maintainer decision):** Single fresh-context agent on **Opus 5.5** at **high** reasoning effort with
  the general review charter below.

## Implementation outline and requirement sources

### Requirement sources

- **User request and decisions:** Full dead-code cleanup + close
  `review_feedback_queue/trailing-existing-environment-checkout-note-dead.md`; no release note highlight needed.
- **Binding repository constraints:** `review_feedback_queue/AGENTS.md` ("Lifecycle" — read in full before editing
  `review_feedback_queue/`), `AGENTS.md` ("Changelog fragments", "Tests and validation"), `.agents/validation.md`,
  `.agents/test-evidence.md`.
- **Planner-proposed implementation choices (reviewed by fresh-context planning review):**
  - Keep `Armed.source` and `submit_or_wait_for_preview` untouched (shared across modes).
  - Because this PR is a pure dead-code cleanup with no user-visible effect (and `replace-checkout-lifetime.md` was
    already consumed by `v0.46.0`), use a `chore:` or `refactor:` Conventional Commit title (no new changelog fragment
    needed).

### PR shape

- **1 PR (`plan/delete-dead-replace-checkout-ui/01-remove-dead-replace-checkout-code`):** Remove the dead launcher code,
  delete `review_feedback_queue/trailing-existing-environment-checkout-note-dead.md` and its `INDEX.md` line, and remove
  the `TODO.md` entry.

## Scope reassessment

Before implementing a substantial departure from the outline above, run a fresh-context review at the executing
session's own model and effort with the following charter verbatim. Supply the original request, settled decisions,
outline, current diff, and proposed departure (what changed, why it is necessary, and which simpler alternative was
ruled out). A close reuse-versus-duplication call not foreseen during planning is a routine fork when every option stays
within the agreed scope: decide after reassessment and log as `DECISION`. Repeated corrective reviews of the same
component also require reassessing necessity before another repair round. Give any same-model sub-agent bounded units
within the outline; unapproved new mechanisms are outside its task.

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

## The checkout

This goal runs inside the checkout the executing session runs in, identified as the nearest Git worktree or jj workspace
containing the session's working directory whose remotes include `scode/farhelm` (comparing host, owner, and name,
ignoring `.git` suffix and SSH vs. HTTPS). Every repository path in this file is relative to that checkout's root.
Confirm this first; if the nearest checkout is not `scode/farhelm`, stop before the resume protocol, write nothing
(including the log), and report that the goal must be run from a checkout of `scode/farhelm`.

## Resume protocol

Right after confirming the checkout, resolve the plan's working log directory with
`cargo xtask records path plan/delete-dead-replace-checkout-ui` (`plans/.agents/outside-files.md`, "Files outside the
repository"). Its `log.md` is the working log. Invoke the `agent-resumeable` skill with that resolved path: if `log.md`
already exists, read it in full and resume where the previous holder left off (cross-checking against VCS state and open
`plan/delete-dead-replace-checkout-ui/*` PRs on GitHub); if it does not exist, this is a fresh start.

## Execution mode

Run in **no-workhorse** mode. Do not send any unit of the executing agent's own decomposition to a weaker model or lower
reasoning effort than the executing session's own. Do not invoke, load, or follow `scode-galaxy-brain` or
`scode-model-routing` at any point; if `scode-galaxy-brain` is already active, this goal file is the maintainer's
express instruction to stop it.

Same-model sub-agents at the executing session's own model and effort are allowed when they are likely to finish work
sooner without costing quality; concurrent writers may use isolated worktrees or jj workspaces that the executing agent
merges back, while the executing agent owns the `jjstack` stack and every PR. The fresh-context read-only sub-agents
required by `plans/AGENTS.md`, "Sub-agents this file requires" (resume check, cold reads of Blocked questions and
reports), the scope reassessment review (at the executing session's own model and effort), and the per-PR review gate
below are explicitly required and exempt from any no-delegation restriction. Launch any sub-agent whose demanded model
is not available natively through `$scode-harness-shellout`.

## Resource watchdog

Mandatory before starting implementation or delegations. Resolve the `scode-build-goal` skill's `scripts/watchdog` path
via `realpath` at run time. Put the watchdog status file at `farhelm-plan-delete-dead-replace-checkout-ui-watchdog.json`
in the parent directory of the executing checkout (derived per `scode-build-goal`, "Placing the files") — never inside
the `plan/delete-dead-replace-checkout-ui` record directory.

- **Watched paths (`--path`):** the executing checkout root, `$HOME`, `/tmp`, the directory holding the status file, the
  executing session's scratch directory, and any worktree/build directory on another filesystem (restart the sampler if
  added). Interval: 60 seconds; default alert levels.
- **Harness setup & verification:** Before starting the real monitors, verify the delivery mechanism once per session on
  throwaway status files in the scratch directory with `--interval 5` (`--synthetic-alert`, killing via status `pid`,
  and `--stall-after 2` watched by an observer armed first). On Claude Code, run `observe` and `sample` under separate
  Monitors (30-minute timeout, observer first); on Codex, arm `observe --exit-on-event` then `sample` through
  `skillette-codex-wake-on-exit`; otherwise log the fallback, run the sampler in the background, and run
  `watchdog check --status <file>` before every launch and at natural boundaries.
- **Acting on shortages/failures & lifecycle:** On `ALERT`/`WORSE` or failing `check`, pause launches, clean run-owned
  build leftovers/worktrees, and resume only once `check` passes (if nothing run-owned can free enough, block/unclaim
  per `plans/.agents/executing.md`). Record monitor handles, watched paths, and delivery mechanism in the working log;
  on resume, stop any still-live prior monitors before starting new ones; stop owned monitors when the plan closes.

## Stack and PR discipline

Build a linear stack of draft PRs using the `jjstack` skill on top of `main@origin` (or this plan's own open
`plan/delete-dead-replace-checkout-ui/*` PRs when resuming, never another plan's PRs), set up per
`plans/.agents/executing.md`, "Executing one plan". Use bookmark names
`plan/delete-dead-replace-checkout-ui/<nn>-<short-name>`. Keep every PR a draft — never mark a PR ready and never merge.
Restructure existing PRs in the stack instead of stacking fixup commits on top, including PRs created by an earlier run.
Never edit anything under `plans/`.

Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments", and "Tests and validation", plus
`.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`. The last code PR removes the
covered `TODO.md` entry.

## Review gate

Before finishing any PR in this stack, delegate an independent fresh-context review to a single agent on **Opus 5.5** at
**high** reasoning effort (explicit model and effort demand; launch natively if supported or via
`$scode-harness-shellout` without hardcoding a CLI command here). Give the reviewer no conversation context from the
executing session; supply the repo root of the executing checkout, the commit range or bookmark to review, the private
findings file path in the executing session's scratch directory, `.agents/test-authoring.md` verbatim when tests or
fixtures changed, and this charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved. Report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

Address all valid findings and re-verify before moving on.

## Decision logging

Log major design decisions in the plan's working log as they happen with a scannable `DECISION` label, noting the
alternatives considered.

## Unattended fallback

This plan runs unattended. Never ask the maintainer anything in conversation — not a question, not an approval prompt,
not an optional offer, and not a harness question tool prompt (`plans/AGENTS.md`, "Unattended means no questions").
Resolve routine implementation forks within the agreed scope, log consequential choices as `DECISION`, and carry on. If
a required decision goes beyond the agreed scope, weakens a guarantee, or hits an unresolvable conflict, block per
`plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Put non-blocking notes and possible follow-ups
in the closing report.

## Done criterion

Every PR in the stack exists on GitHub as a draft, has passed its Opus 5.5 high review gate and applicable validation,
removes the covered `TODO.md` entry in the last code PR, and satisfies the latest `## Decisions` entry (if any).
Delivering the report is the executor's closing step (`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

# Goal: Wait for in-flight hook-setup saves before closing Settings

Make closing Settings wait for an in-flight immediate toggle save (`toggle_configured`, such as turning off automatic
conversation hooks for Grok or Antigravity) to settle before closing — staying on that agent's Settings page showing the
inline error if the save fails — and record the maintainer's general low-latency-helm rule for Settings edits in
`SPEC.md`.

**Source TODO entry (removed in the last code PR):** `Show a failed hook setup save after Settings closes.` under
`## Plan follow-ups` in `TODO.md`.

## Acceptance criteria

- `SPEC.md` ("Settings") records the maintainer's rule: for Settings edits and saves, Farhelm presumes a low-latency,
  ordinarily reliable helm; Settings does not add asynchronous workarounds or surviving out-of-dialog notice machinery
  just to let the user move on before a helm round trip settles, and waiting for a helm round trip before closing or
  moving on after a save is expected.
- When `toggle_configured` has a save in flight and the user attempts to close Settings (via header × or Escape, whether
  on a plain Close or with other staged drafts dirty), Settings waits for that save to settle instead of dropping the
  dialog early.
- If the toggle save succeeds, Settings continues closing (or opens the normal "Save changes before closing?" prompt if
  other staged drafts are dirty).
- If the toggle save fails, Settings stays open on that section with the inline error visible (without duplicating the
  write, without opening the close confirmation, and without sending staged drafts).
- No new window notice or bell notification surface is added.
- GUI tests cover plain Close racing both a succeeding and a failing hook-setup opt-out, and Close with another staged
  draft racing a failing hook-setup opt-out.

## Decisions already made

- **Wait on Close instead of notice/bell:** Wait for the in-flight toggle save before closing Settings; stay on the page
  with the inline error if it fails.
- **Maintainer's `SPEC.md` rule (preserve the substance verbatim in `SPEC.md`):**
  > For the purposes of settings edits/saves, we treat the Helm as a presumed low latency infallible remote. We do not
  > add asynchronous flows to settings editing flows to account for helm being far away. It's okay to make the user wait
  > for a helm round trip before they can move on after "save" or similar operation. We do not add complexity just to
  > allow the user to move on if they don't have solid low latency connectivity to their helm.
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter below.

## Implementation outline and requirement sources

- **User request & decisions:** `TODO.md` ("Show a failed hook setup save after Settings closes.") plus the settled
  wait-on-Close decision and `SPEC.md` rule above (which supersede the executor-proposed surviving window notice and
  multi-form `Batch` wiring mentioned in the `TODO.md` note).
- **Binding repository constraints:** `SPEC.md` ("Settings") and `SPEC_impl.md`.
- **Planner-proposed implementation choices (scope-reviewed):**
  - Keep immediate switches (`toggle_configured` in `crates/gui/farhelm-gui/src/settings.rs`) completely separate from
    `settings/editing.rs::Edit` and `Batch` (do not merge them or touch `SaveReceipt` / `AfterBatch` /
    `watch_close_question`).
  - Because every `toggle_configured` switch saves under `&OpKey::preferences()`, at most one configured-switch save is
    in flight at a time. Track `pending_configured_save: Option<Section>` and `close_after_configured_save: bool` on
    `SettingsDialog` (cleared on user navigation/edits or settlement).
  - At the top of `ask_to_close` (`settings/editing.rs:753`), before checking staged drafts or opening a close question:
    if `self.pending_configured_save.is_some()`, set `self.close_after_configured_save = true` and return
    `Dismissal::Stay`.
  - When `toggle_configured`'s `drafts::save` callback settles while `close_after_configured_save` was set: on
    `Outcome::Done(_)`, re-invoke `self.ask_to_close(window, cx)` (which finishes plain Close or opens the normal
    staged-drafts prompt); on refusal, call `self.reveal_save_failure(section, window, cx)`. This covers `hooks`,
    `trust`, and `spare` toggles uniformly in ~25 lines.

## Scope reassessment

Before implementing a substantial departure from the outline, run a fresh-context review at the executing session's own
model and effort with this charter:

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

Supply the request, decisions, outline, current diff, and proposed departure. Repeated corrective reviews of the same
component also require reassessing its necessity before another repair round. Give same-model sub-agents bounded units
within the outline; unapproved new mechanisms are outside their task.

## The checkout

This goal runs inside the checkout the executing session runs in, identified by GitHub repository `scode/farhelm` (the
nearest Git worktree or jj workspace containing the working directory; never identified by path). Confirm first that the
nearest checkout has a remote pointing at `scode/farhelm`; if not, stop before the resume protocol, write nothing, and
report the mismatch. Every project file path in this plan is repo-relative.

## Resume protocol

Right after the checkout check, resolve the plan's record with `cargo xtask records path plan/settings-hook-save-notice`
(`plans/.agents/outside-files.md`, "Files outside the repository") and invoke the `agent-resumeable` skill with that
directory's `log.md`. If `log.md` already exists, read it in full and resume where the previous session left off —
cross-checking against GitHub and jj state per `plans/.agents/executing.md`, "Executing one plan" — rather than starting
over. If it does not exist, this is a fresh start.

## Execution mode

Explicit **no-workhorse mode**: do not send any unit of your own decomposition to a weaker model or lower effort than
your own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point (if
`scode-galaxy-brain` is active when the run starts, stop it). Same-model sub-agents at your own model and effort are
allowed whenever they finish work sooner without costing quality, using separate worktrees or jj workspaces if
concurrent writers might conflict. The review gate keeps the explicit Opus 5.5 high demand below, scope reassessment
runs at your own model and effort, and the required sub-agents listed in `plans/AGENTS.md`, "Sub-agents this file
requires" (resume check, cold reads of Blocked questions and reports), are exempt from any no-delegation restriction.
Launch any sub-agent whose model is unavailable natively through `scode-harness-shellout`.

## Resource watchdog

Right after the resume protocol and before any delegation, start the `scripts/watchdog` Deno script from the installed
`scode-build-goal` skill directory (resolved with `realpath` at run time) to monitor memory and disk for the rest of the
run:

- **Status file:** `farhelm-plan-settings-hook-save-notice-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files", never inside the `plan/settings-hook-save-notice`
  record directory).
- **Watched paths (60s interval, default alert thresholds):** the executing checkout root, `$HOME`, `/tmp`, the parent
  directory holding the status file, the session's scratch directory, and any temporary worktree or build directory
  created on another filesystem.
- **Verification & lifecycle:** Verify the delivery mechanism on throwaway status files in scratch space
  (`--interval 5`, `--synthetic-alert`, killed pid, `--stall-after 2`) before arming the real monitors (Claude Code dual
  Monitors; Codex `wake-on-exit`; plain background fallback with `watchdog check` before launches and at boundaries if
  neither is available). Run `watchdog check --status <file>` before every new workload launch and at natural
  boundaries. Act immediately on `ALERT`/`WORSE`/`STALE`/`ERROR` by pausing new launches and freeing run-owned build
  artifacts; stop owned monitors at close.

## Stack, PR discipline, and repository conventions

- Build a linear stack of bite-sized draft PRs on top of `main@origin` (or this plan's own open PRs when resuming, never
  another plan's PRs) using the `jjstack` skill, set up per `plans/.agents/executing.md`, "Executing one plan".
- Name bookmarks `plan/settings-hook-save-notice/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never
  merge.
- Restructure existing PRs in the stack (including ones from an earlier run) instead of stacking corrections on top.
- Never edit anything under `plans/`.
- The last code PR removes `Show a failed hook setup save after Settings closes.` from `TODO.md`.
- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation") and
  `.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md` (supply
  `.agents/test-authoring.md` verbatim to the reviewer when tests or fixtures change).

## Review gate

Before finishing any PR in the stack, run a review by a single fresh-context **Opus 5.5** agent at **high** effort
(explicit demand for model and effort; launch natively or via `scode-harness-shellout` without hardcoding a CLI command
here). Give the reviewer the repo root, the commit range or bookmark to review, the private findings file path in
scratch space, and `.agents/test-authoring.md` verbatim if tests changed, with this charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

Address what it finds before moving on.

## Decision logging and unattended fallback

Log consequential design choices in the working log as `DECISION` with alternatives considered. Resolve routine
implementation forks within agreed scope and log them; remove unnecessary planner-invented machinery when agreed
behavior and repository constraints still hold.

Never ask the maintainer anything in conversation — not a direct question, not a harness question tool prompt, not an
optional offer or closing question (`plans/AGENTS.md`, "Unattended means no questions"). Every fork is either decided
within scope and logged as `DECISION`, or — when a material scope expansion, weakened guarantee, or maintainer-only
product choice cannot be resolved from the plan, its `## Decisions`, and the specs — blocked per
`plans/.agents/executing.md`, "Executing one plan", step 10. Things the maintainer should know and optional follow-ups
go into the report.

## Done criterion

Every PR in the stack exists on GitHub, has passed its Opus 5.5 high review gate, removes the covered `TODO.md` entry in
the last code PR, and satisfies the latest `## Decisions` entry (if any). Delivering the report is the executor's
closing step, not part of this goal.

## Release note

No release note needed.

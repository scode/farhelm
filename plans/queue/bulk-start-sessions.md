# Goal: Start several selected sessions at once

Add a bulk `Restart/Replace` action to the marked-sessions bar in the desktop app so that when all marked sessions are
stopped or ended (such as after a remote host reboot), you can start all of them together in one click instead of
clicking through each session individually.

**Source TODO entry (removed in the last code PR):** `Start several selected sessions at once.` under `## Immediate` in
`TODO.md`.

## Acceptance criteria

- Whenever every displayed marked session is in an ended or post-reboot state (`Stopped`, `Exited`, `Interrupted`,
  `Error`, or `terminal_missing` — and any other ended state that occurs after a host reboot) and at least one marked
  session can be restarted or replaced, the marked-sessions bar offers a `Restart/Replace` button beside `Stop`.
- The button's hover help says `Restart when possible, Replace when not`.
- Clicking `Restart/Replace` runs immediately with no confirmation modal, reusing the existing per-host sequential bulk
  queue (`crates/gui/farhelm-gui/src/session_list/bulk.rs`) so hosts run in parallel while sessions on the same host are
  paced sequentially.
- For each admitted target, if `restart::offered(row)` is true (`RestartOffer::Resume`), Farhelm dispatches
  `Work::Restart` (resuming the saved conversation); otherwise Farhelm dispatches `Work::Replace` (starting a fresh
  session with the same launch/folder). Never silently replace a session that can resume.
- Rows show in-flight `Restarting…` progress (matching existing `Replacing…` progress) while `Work::Restart` is running.
- Unstartable rows (retired sessions, unreachable hosts, frozen/deleting environments, busy rows, or rows with an open
  confirmation) are handled cleanly with existing per-row `not_started` / outcome reporting without blocking the
  remaining targets; the button honors `menu::launch_wait(cx)` while a host change is in flight.
- `SPEC.md` ("Session list" and "Lifecycle operations") documents bulk `Restart/Replace`, and GUI tests cover
  eligibility, mixed resumable/non-resumable batches, per-host pacing, row progress, and refusals.

## Decisions already made

- **Ended/post-reboot scope:** Cover all ended states (`Stopped`, `Exited`, `Interrupted`, `Error`) plus
  `terminal_missing` and any other post-reboot non-running state. The primary use case is restarting a batch of sessions
  after a host reboot.
- **Mixed resumable and non-resumable sessions:** Offer a single `Restart/Replace` action whose hover help says
  `"Restart when possible, Replace when not"`.
- **Label and placement:** Place `Restart/Replace` beside `Stop` in the marked-sessions bar (keeping `Stop` available
  because ended sessions can still have open shell tabs).
- **Confirmation:** Run immediately on click without a confirmation modal (matching single-session Restart on an ended
  session).
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter below.

## Implementation outline and requirement sources

- **User request & decisions:** `TODO.md` ("Start several selected sessions at once.") plus the settled decisions above.
- **Binding repository constraints:** `SPEC.md` ("Session list", "Lifecycle operations", "One GUI at a time") and
  `SPEC_impl.md` stay authoritative. Do not add client-side YOLO prediction (host YOLO confirmations arrive as
  `HelmMarker::YoloConfirmation` and are already handled per row by `replace::settle_sent` -> `replace_yolo::ask`).
- **Planner-proposed implementation choices (scope-reviewed):**
  - **Generalize dispatch entry points:** Extract `restart::start(..., then) -> Result<(), NotStarted>`
    (`crates/gui/farhelm-gui/src/session_list/restart.rs`) and `replace::start(..., then) -> Result<(), NotStarted>`
    (`crates/gui/farhelm-gui/src/session_list/replace.rs`) matching `stop::start` / `delete::start`, keeping `send` as
    thin wrappers so bulk execution reuses `menu::refuse_for_host_change`, `outcome::start_clock`,
    `card::restart_finished`, and `replace::settle_sent` without duplicating them.
  - **Reuse `Batch` without bloating `bulk::Lifecycle`:** Keep `bulk::Lifecycle` / `bulk::Target` for confirmable
    destructive actions (`Stop`, `Delete`), and have `Batch::queues` hold a thin `Queued` item (`key`, `host`, `whole`,
    `op: QueuedOp { Stop, Delete, EnvironmentDelete, Restart, Replace }`) so non-confirming `Restart/Replace` reuses
    per-host `VecDeque` pacing, `Operations::hold`/`release`, and `Marks::revision` pruning without dead confirmation
    arms.
  - **Generalize row wait line:** Replace `replace::progress_line` with one `outcome::row_wait_line` covering
    `Work::Replace | Work::Restart` (reusing `outcome::wait_line`, which already formats
    `"Restarting on <host>… <n> s"`) so `session_list/row.rs` has one progress slot.
  - **Projection & bar:** Extend `session_list/marking.rs::Projection` to expose bulk `Restart/Replace` eligibility
    (honoring `menu::launch_wait(cx)`), render the button beside `Stop` with its hover help, and update `SPEC.md` + GUI
    goldens/tests as needed.

## Scope reassessment

Before implementing a substantial departure from the outline (such as a new subsystem, recovery protocol, compatibility
layer, or mechanism that duplicates or broadly generalizes an existing one), run a fresh-context review at the executing
session's own model and effort with this charter:

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

Right after the checkout check, resolve the plan's record with `cargo xtask records path plan/bulk-start-sessions`
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

- **Status file:** `farhelm-plan-bulk-start-sessions-watchdog.json` in the parent directory of the executing checkout
  (derived per `scode-build-goal`, "Placing the files", never inside the `plan/bulk-start-sessions` record directory).
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
- Name bookmarks `plan/bulk-start-sessions/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
- Restructure existing PRs in the stack (including ones from an earlier run) instead of stacking corrections on top.
- Never edit anything under `plans/`.
- The last code PR removes `Start several selected sessions at once.` from `TODO.md`.
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

You can now restart multiple stopped sessions at once. Select several stopped sessions in the session list and use the
Restart button in the selection bar to resume all of them together. This makes resuming work after a remote host
rebooted considerably less infuriating!

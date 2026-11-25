# Hooks, Restart, and Antigravity CLI fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

**Dependency:** This plan runs after `drop-post-clone-claude-trust.md` lands on `main`, because PR 3 edits
`exec_launch_spec_with_seam` in `crates/farhelm-supervisor/src/launch.rs` right after launch-time Claude trust and must
write the handoff marker strictly after the trust edit and immediately before `exec()`.

## The goal

Implement three hook and Restart `TODO.md` entries from `## Plan follow-ups`, grouped as one 3-PR linear stack. Every
item is subject to the complexity kick-back rule below. Verbatim:

1. > **Keep Antigravity's Restart selector before a Custom `--`.** With Antigravity 1.5.16, a launch with
   > `--mode auto --settings '{}' --` reaches the composer, completes a turn and captures a conversation. The saved
   > command places `--conversation` after `--`, where it is a positional argument. Keep the settings and conversation
   > options before the delimiter, preserving stored Custom words and workspace-trust edits. The `permission_modes`
   > drift case reports this boundary before it would start the wrong conversation.

2. > **Isolate hook setup errors in Restart and recovery.** A failed persistent hook setup carries the host's error in
   > `PersistentHookSetup::Failed`. `restart_blocker` escapes and joins it into app prose before the desktop can isolate
   > its direction. Keep the raw error separate until drawing. Pass it through `HostText` and `HostSentence` for the
   > session header's Restart button tooltip and its Restart and Restart with menu entries, and for the interrupted
   > session's recovery card and Restart tooltip. Follow `SPEC_impl.md`'s Host-supplied text rule and keep the CLI's
   > terminal wording. Test a right-to-left reason and hidden characters against the surrounding app text in the drawn
   > UI and its accessible descriptions. The current helpers pass plain strings through the launcher and recovery card,
   > so this needs typed composition and tests across the surfaces. The triage plan deferred it under its size gate.

3. > **Start the missing-hook warning clock after the agent starts.** In a fresh managed checkout's terminal, pressing
   > Enter during the clone or post-clone command can start the missing conversation-hook timer. SPEC.md's Status
   > section ties the warning to a submitted line the agent received. Keep sending input during preparation. In
   > `service/connection.rs` and `service/capture.rs`, arm the timer on the first accepted Enter after generation-bound
   > evidence that the shim handed off to the agent. The launcher waypoint marks an earlier shell step and checkout
   > Ready precedes the trust edit and agent exec. A reliable handoff signal needs recovery and cleanup rules so a prior
   > launch cannot arm this one. Test that a preparation Enter stays quiet after the deadline and that an Enter
   > delivered to the agent can still start the once-per-launch warning. The triage plan deferred the handoff work and
   > its lifecycle tests.

### Acceptance criteria

Each implemented item removes its own `TODO.md` entry in its PR:

1. **PR 1 (Antigravity Restart selector before Custom `--`):**
   - Compiled Antigravity resume commands (ordinary Restart and `named_start`) place `--settings` and `--conversation`
     before any Custom `--` delimiter, preserving Custom words and workspace-trust edits (`with_workspace_trust`).
   - `with_workspace_trust` continues accepting pre-fix stored sessions (`resume.starts_with(old_prefix)`) alongside the
     new layout so cloning/spawning older sessions still works.
   - Verified with unit tests in `launches.rs`, `workspace_trust.rs`, and the drift check boundary in
     `crates/farhelm/tests/e2e/real_agent_capture/drift/permission_modes.rs`.
2. **PR 2 (Isolate hook setup errors in Restart and recovery):**
   - `PersistentHookSetup::Failed` raw reasons stay separate from app prose until draw time, rendered via `HostText` and
     `HostSentence` (`HoverHelp::sentence`, `MenuItem::description_sentence`) across the session header's Restart button
     tooltip, Restart and Restart with menu entries, and the interrupted-session recovery card and Restart tooltip.
   - CLI terminal wording stays unchanged, sharing the same structured prose parts with the GUI.
   - Tests cover RTL reasons and hidden characters in both drawn UI and accessible descriptions.
3. **PR 3 (Delay missing-hook warning clock until agent handoff):**
   - Input during checkout preparation continues to be sent to the terminal, but Enter before the shim hands off to the
     agent never arms the missing-hook timer.
   - The shim writes a generation-scoped handoff marker (e.g., `.handoff`) in `LaunchArtifacts` immediately before
     `exec()` (after checkout preparation, directory entry, and launch-time Claude trust).
   - `StoredSession` records an upgrade-safe `handoff_marker_expected` boolean column (migrated default `false`) so live
     pre-upgrade sessions reloaded after upgrade initialize `handoff_seen = true` and keep arming on Enter, while
     post-upgrade sessions mid-preparation reload with `handoff_seen = false`.
   - The marker participates in `parse_launch_file_name`, `clear_for_retake_fail_closed`, and `cleanup_launch_artifacts`
     so prior/retaken generations can never arm a new launch.
   - Filesystem checks never run while holding `sup.attachments`; tests prove preparation Enter stays quiet past the
     deadline while an Enter after handoff arms the once-per-launch warning.

### Complexity kick-back rule (applies to every item in this plan)

The maintainer's instruction for `Plan follow-ups` items: _"If this ends up being complicated, kick it back to the to-do
queue. The executor should not continue if this turns very complicated."_ Evaluate each item independently. If any item
turns very complicated during execution, stop work on that item instead of pushing through: leave its `TODO.md` entry in
`## Plan follow-ups` updated with concrete findings explaining what made it complicated, log a `DECISION`, continue with
the remaining items in this plan, and summarize any kicked-back item in the delivery report.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped into one plan:** Approved by maintainer.
- **Missing-hook timer handoff mechanism:** Generation-scoped marker in `LaunchArtifacts` + upgrade-safe flag so
  pre-upgrade live sessions keep arming on Enter.
- **Complexity kick-back:** Applies to all three items individually.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Code rules": read `crates/farhelm-supervisor/src/agent_kind/mod.rs` before changing per-harness behavior).
- Follow `.agents/test-authoring.md`, `.agents/validation.md`, and `.agents/test-evidence.md`. Never mutate process
  environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`crates/farhelm-helm/src/launches.rs`, `workspace_trust.rs`):**
   - Reuse `trust_custom_tail_len(&selection)` (optionally renaming to `custom_positional_tail_len`) in `compile` so
     `resume_arguments` are spliced at `resume.len() - tail_len` (fixing both ordinary Restart and `named_start`).
   - In `with_workspace_trust`, the trust slice `trust_start..trust_end` sits at the exact same index range in both the
     legacy and new layouts; update only the structural prefix check to accept either layout.
2. **PR 2 (`crates/farhelm-proto/src/lib.rs`, `crates/gui/farhelm-gui/src/session_list/restart.rs`, `launcher/entry.rs`,
   `session_pane/header.rs`, `session_pane/card.rs`):**
   - Add one small structured parts helper on `PersistentHookSetup` in `farhelm-proto` (`NotRequested` vs `Failed`) and
     implement `restart_blocker` (for CLI wording) in terms of it so prose never drifts between CLI and GUI.
   - Add one `append_restart_blocker(HostSentence, ...)` helper in `session_list/restart.rs` shared by `restart.rs` and
     `launcher/entry.rs`, and pass `HostSentence` into `HoverHelp::sentence` and `MenuItem::description_sentence` across
     `session_pane/header.rs` and `session_pane/card.rs`.
3. **PR 3 (`crates/farhelm-supervisor/src/launch.rs`, `service/launch_artifacts.rs`, `service/core.rs`,
   `service/connection.rs`, `service/capture.rs`, `store.rs`):**
   - Do **not** add an expectation flag to `LaunchSpec` (it is unlinked by `consume_spec` before preparation/`exec`, and
     the new shim writes the handoff marker unconditionally right before `exec()`).
   - Add a single `handoff_marker_expected: bool` column to `StoredSession` (migrated default `0`/`false`, set `true` in
     the existing create/relaunch insert/update statements, restored on aborted-restart rollback like `launch_hooked`).
   - Add the marker path (e.g. `.handoff`, created with `O_CREAT | O_EXCL | O_NOFOLLOW` like `.waypoint`) to
     `LaunchArtifacts`, `parse_launch_file_name`, `clear_for_retake_fail_closed` (mandatory so retaken generation 0
     cannot inherit a prior marker), and `cleanup_launch_artifacts`.
   - On `SessionRun`, keep an `AtomicBool` latch `handoff_seen` (initialized `!handoff_marker_expected` on reload,
     `false` on fresh create/relaunch). In `connection.rs`, short-circuit before any filesystem check unless
     `TerminalId::Agent`, `send_result == Some(Ok(true))`, `before_input != ScreenState::Waiting`, `entry.run.hooked`,
     `first_input.is_none()`, and `!handoff_seen`. Drop `sup.attachments` before running the non-blocking
     `symlink_metadata` check (`is_file()`), then latch `handoff_seen` and call `note_first_input`.

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
`farhelm-plan-hooks-restart-and-antigravity-fixes-log.md` in the parent directory of the executing checkout. If the log
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
`plan/hooks-restart-and-antigravity-fixes/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge.
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
`farhelm-plan-hooks-restart-and-antigravity-fixes-watchdog.json` beside the working log, watching the executing checkout
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

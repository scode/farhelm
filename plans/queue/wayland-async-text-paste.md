# Goal: Make Wayland text-field paste nonblocking

Make pasting into desktop text fields on Linux Wayland asynchronous and text-first so a slow foreign clipboard provider
never freezes the window, while preserving file- and image-first paste for terminals and reusing GPUI Kit's existing
async paste handling.

**Source TODO entry (removed in the last code PR):** `Make Wayland text-field paste nonblocking.` under
`## Plan follow-ups` in `TODO.md`.

## Acceptance criteria

- Pasting into desktop text fields (via keyboard shortcut or the Edit menu, including SSH sign-in prompts) does not
  block the Wayland main thread on uncached foreign clipboard offers.
- Text fields prefer text on mixed clipboard offers (such as image+text or file+text), preserving GPUI Kit's focus,
  document-revision, selection, newline, and atomic-undo checks.
- Terminal paste (`crates/gui/farhelm-gui/src/terminal/attached.rs`) keeps file-first / image-first / text-fallback
  async paste behavior.
- Any edits to vendored GPUI (`vendor/gpui-pre*`) are recorded in their patch files under `vendor/patches/gpui/` and
  `vendor/README.md` per `vendor/README.md`.
- Tests cover keyboard and Edit-menu paste, mixed clipboard offers, and an async clipboard reply arriving after the
  target text field or selection changed.

## Decisions already made

- Use the vendored-GPUI clipboard API split (reusing `gpui-base`'s built-in async paste fallback) rather than
  intercepting and duplicating Kit's paste state machine in `farhelm-gui`.
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter below.

## Implementation outline and requirement sources

- **User request & decisions:** `TODO.md` ("Make Wayland text-field paste nonblocking.") and `SPEC.md` ("Terminal
  experience": Farhelm must stay responsive during clipboard operations on Wayland).
- **Binding repository constraints:** Read `vendor/README.md` in full before editing anything under `vendor/`; every
  vendored change must keep its patch file in sync. Remember `.agents/validation.md`'s vendored GPUI test selection and
  macOS host-text note when touching GPUI patches.
- **Planner-proposed implementation choices (scope-reviewed):**
  - In `vendor/gpui-pre`, `vendor/gpui-pre-linux`, and `vendor/gpui-pre-macos` (and their corresponding patch files
    under `vendor/patches/gpui/`):
    - Make `read_from_clipboard_async()` **text-first** (matching upstream GPUI and `gpui-base`'s expectation).
    - Add `Platform::read_from_clipboard_payload_async()` with a default trait implementation delegating to
      `self.read_from_clipboard_async()` (avoiding boilerplate on X11, `TestPlatform`, and `VisualTestPlatform`),
      overriding it on Wayland and macOS for files -> images -> text.
    - On Wayland (`vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs`), have synchronous `Clipboard::read()` return
      only local/cached items and `None` for uncached foreign offers so `gpui-base`'s `InputState::paste()` falls
      through to its existing `read_from_clipboard_async()` path automatically.
  - Switch the single terminal paste caller (`crates/gui/farhelm-gui/src/terminal/attached.rs`) to
    `read_from_clipboard_payload_async()`.
  - Add targeted tests for Wayland nonblocking text-first paste, mixed offers, Edit-menu paste, and late replies after
    target/selection changes.

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

Right after the checkout check, resolve the plan's record with `cargo xtask records path plan/wayland-async-text-paste`
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

- **Status file:** `farhelm-plan-wayland-async-text-paste-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files", never inside the `plan/wayland-async-text-paste` record
  directory).
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
- Name bookmarks `plan/wayland-async-text-paste/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never
  merge.
- Restructure existing PRs in the stack (including ones from an earlier run) instead of stacking corrections on top.
- Never edit anything under `plans/`.
- The last code PR removes `Make Wayland text-field paste nonblocking.` from `TODO.md`.
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

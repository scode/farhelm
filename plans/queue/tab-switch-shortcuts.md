# Tab-switching keyboard shortcuts

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them (the spec updates below are part of the work).

## The goal

Implement one `TODO.md` entry from the `## Immediate` bucket. Verbatim:

> **Cmd-Shift-[ and ] to switch tabs.** Add cmd-shift-[ and ] to switch between terminal tabs/agents within a session.

### Acceptance criteria

1. **Shortcuts switch between terminal tabs in an open session:**
   - `Cmd+Shift+[` / `Cmd+Shift+]` on macOS and `Ctrl+Shift+[` / `Ctrl+Shift+]` on Linux step to the previous / next
     terminal tab within the open session (including the main Agent/Shell tab and all live shell tabs, skipping any tab
     in `confirmed_closed`).
   - Stepping wraps around between the main terminal tab and the last live shell tab.
   - When the session has 0 live shell tabs (only the main terminal), or when the terminal set is inert/unavailable, or
     when no session is open, or when an interrupted/retired/unavailable card is showing, the shortcuts are clean no-ops
     that steal no focus and leak no keystrokes to the terminal program.
   - Switching works from anywhere in the main window while no modal/focus layer is open and no terminal IME composition
     is active, focuses the newly chosen terminal via `TerminalSet::choose`, scrolls the chosen tab button into view in
     the tab strip if overflowed, and allows held-key auto-repeat.
   - Keystrokes are intercepted before terminal or text-field listeners so neither `[`/`]` nor `{`/`}` leaks into the
     running program.
2. **Shortcuts overview, specs, docs, and tests:**
   - Add the two shortcuts under the `Terminal` section of the Keyboard shortcuts dialog (`shortcuts.rs`) and keep the
     keymap drift test passing.
   - Update `SPEC.md` (keyboard shortcuts / session view) and the docs pages (`keyboard-shortcuts.mdx` and
     `work-in-a-session.mdx`).
   - Add unit tests covering physical-key matching on macOS and Linux, wrap-around in both directions, skipping
     `confirmed_closed` tabs, 0-shell-tab and inert no-ops, modal/IME guards, and terminal focus transfer.
   - Remove the covered `TODO.md` entry in the final code PR.

## Decisions already made

### Maintainer decisions (binding)

- **Linux chord:** `Ctrl+Shift+[` and `Ctrl+Shift+]` (matching every other Linux shortcut in Farhelm).
- **Wrap around:** Wrap between the main terminal tab and the last live shell tab.
- **Focus scope and repeat:** Work anywhere in the window outside dialogs/IME composition, focus the chosen terminal,
  allow held-key repeat, and no-op when there are 0 shell tabs.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** Settled verbatim in `## Release note` below.

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md")
  and `.agents/test-authoring.md` / `.agents/validation.md` / `.agents/test-evidence.md`.
- GPUI drops `shift` from logical keystrokes for shifted symbols (`Shift+[` becomes `{`), which is why shifted-symbol
  shortcuts use `PHYSICAL_KEYS` in `crates/gui/farhelm-gui/src/actions.rs`.
- Never mutate process environment variables in tests; annotate any test sleep with `// sleep-ok:` passing
  `scripts/check-test-sleeps.py`; never touch the live Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **Actions (`crates/gui/farhelm-gui/src/actions.rs` + `actions_tests.rs`):** Add `PreviousTerminalTab` and
   `NextTerminalTab` to `PHYSICAL_KEYS` using physical US `[` / `]` positions (`AD11`/`0x21`/`"["` and
   `AD12`/`0x1E`/`"]"`) with `COMMAND_SHIFT` on macOS and `CONTROL_SHIFT` on Linux (same mechanism as
   `IncreaseTerminalTextSize` / `DecreaseTerminalTextSize`). Do not add them to `ONCE_PER_PRESS`, so held keys repeat.
2. **Tab stepping (`crates/gui/farhelm-gui/src/terminal/set.rs`):** Add a stepping method on `TerminalSet` that builds
   the ordered live tab list (`None` for the main terminal, followed by non-`confirmed_closed` shell tab IDs), returns
   early without side effects when `self.inert` or when no shell tabs exist, computes the wrapped target index, calls
   `self.choose(target_id, mark, window, cx)`, and calls `self.scroll.scroll_to_item(index)` directly (0 for main,
   `n + 1` for shell tab `n`) without introducing a second reveal-tracking field beside `TerminalSet::revealed`.
3. **Dispatch (`crates/gui/farhelm-gui/src/main_view.rs` & `session_pane.rs`):** Register `.on_action` handlers **only**
   on `MainView`'s ground div in `main_view.rs` (matching all other window-wide shortcuts; `session_pane.rs` has no
   `.on_action` handlers), guarding on `layers::focus_layer_open(window, cx)` and
   `main.session_pane.read(cx).terminal_is_composing(window, cx)`, and delegate to a plain forwarding method on
   `SessionPane` (`step_terminal_tab`) that forwards to `mounted.set`.
4. **Shortcuts overview & docs (`crates/gui/farhelm-gui/src/shortcuts.rs`, `SPEC.md`, `website/`):** Reuse the existing
   shifted-modifier closure (`ChordModifiers { command: mac, control: !mac, shift: true }`) in `shortcuts.rs`, add both
   rows under `Terminal`, update `shortcuts/tests.rs`, update `SPEC.md`, and update
   `website/src/content/docs/docs/using/keyboard-shortcuts.mdx` and `work-in-a-session.mdx`.

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
`farhelm-plan-tab-switch-shortcuts-log.md` in the parent directory of the executing checkout. If the log already exists,
read it and resume where the previous session left off (cross-checking against VCS state and open PRs) rather than
starting over. If it does not exist, this is a fresh start.

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
`plan/tab-switch-shortcuts/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge. Never edit
files under `plans/`. Within a run, restructure existing open PRs of this plan instead of stacking corrections on top.
Add changelog fragments per `AGENTS.md`, "Changelog fragments", and validate proportionally per `.agents/validation.md`
(including `bun install --frozen-lockfile && bun run build` in `website/` since docs pages change).

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
`farhelm-plan-tab-switch-shortcuts-watchdog.json` beside the working log, watching the executing checkout root, `$HOME`,
`/tmp`, the log's parent directory, your scratch directory, and any worktrees on other filesystems, at a 60-second
interval with default thresholds. Follow the `scode-build-goal` skill's Resource watchdog section for harness wiring
(Claude Code Monitor, Codex wake-on-exit, or fallback), startup verification, acting on alerts, and stopping the
monitors at close.

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

You can now switch between terminal tabs in an open session with Cmd+Shift+[ and Cmd+Shift+].

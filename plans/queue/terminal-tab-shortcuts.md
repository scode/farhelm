# Terminal tab shortcuts (`Cmd+T`, `Cmd+W`, and open-tabs delete warning)

## Goal

Add keyboard shortcuts to open and close individual shell terminal tabs (`Cmd+T` / `Ctrl+Shift+T` to open, `Cmd+W` on
macOS and `Ctrl+Shift+W` on Linux to close only the active shell tab), and always ask for confirmation before deleting a
session that still has open shell terminal tabs — even when "Skip session confirmations" is enabled.

Covers and removes these three `TODO.md` entries in the last code PR (merged into one plan with maintainer agreement):

- Immediate: **Cmd+T to create a new terminal tab.**
- Immediate: **Cmd+W in a terminal tab closes only the tab.**
- Immediate: **Warn before Cmd+W on the main agent kills terminal tabs.**

### Acceptance criteria

- Pressing `Cmd+T` on macOS or `Ctrl+Shift+T` on Linux opens a new shell terminal tab in the active session (reusing the
  `+` button's open path and availability checks). It is a silent no-op when no session is open, tabs are unavailable
  (retired/recovery card/inert/delete in flight), a focus layer (modal, menu, Settings, popover) is open, or IME is
  composing.
- On macOS, pressing `Cmd+W` when a shell terminal tab is selected closes only that shell tab (reusing the tab close
  flow and following "Skip session confirmations": confirm when off, close immediately when on). When on the main agent
  tab (or no shell tab is selected, or the tab strip is inert), `Cmd+W` deletes the session as before.
- On Linux, pressing `Ctrl+Shift+W` closes the active shell terminal tab (no-op when on the main agent tab or no shell
  tab is selected), following "Skip session confirmations", while `Ctrl+Shift+Backspace` remains the Delete session
  shortcut.
- When closing a shell tab via shortcut opens a close confirmation popover, an off-screen tab is scrolled into view
  before anchoring the popover so it stays visibly anchored under the tab's `×` button.
- Deleting any single session that has at least one open shell terminal tab ALWAYS asks for confirmation — whether
  triggered by `Cmd+W`, `Ctrl+Shift+Backspace`, the header Delete button, or the session row menu — even when "Skip
  session confirmations" is enabled (reusing the existing consequence text that counts open terminal tabs). The header
  Delete button tooltip reflects that confirmation will be asked when tabs are open.
- The Keyboard Shortcuts dialog, `+` button tooltip, `SPEC.md`, and user docs (`keyboard-shortcuts.mdx`,
  `work-in-a-session.mdx`, `stop-restart-resume.mdx`, `settings.md`) are updated to match.

## Decisions already made

- **Merged scope (maintainer decision):** Merge `Cmd+T`, `Cmd+W` in a terminal tab, and the open-tabs Delete warning
  into one plan (`terminal-tab-shortcuts`), removing all three Immediate `TODO.md` entries together. (The larger
  "closeable main agent tab" idea was deferred to a separate Near term `TODO.md` entry added in the planning PR.)
- **New terminal tab shortcut (maintainer decision):** `Cmd+T` on macOS and `Ctrl+Shift+T` on Linux; silent no-op when
  no session is open or tabs are unavailable.
- **Close terminal tab shortcut and confirmation (maintainer decision):** macOS `Cmd+W` closes the active shell tab;
  Linux `Ctrl+Shift+W` closes the active shell tab while keeping `Ctrl+Shift+Backspace` for Delete session; both follow
  "Skip session confirmations" (confirm when off, close immediately when on).
- **Open-tabs Delete warning scope (maintainer decision):** Override "Skip session confirmations" on BOTH `Cmd+W` (and
  Linux `Ctrl+Shift+Backspace`) AND clicking Delete (header/row menu) whenever at least one shell terminal tab is open.
- **Review gate (maintainer decision):** Single fresh-context agent on **Opus 5.5** at **high** reasoning effort with
  the general review charter below.

## Implementation outline and requirement sources

### Requirement sources

- **User request and decisions:** All items under "Decisions already made" above, plus the settled combined release note
  below.
- **Binding repository constraints:** `SPEC.md` ("Delete", "Skip session confirmations", "Session view", "Desktop window
  chrome"), `crates/gui/farhelm-gui/src/actions_tests.rs` (no duplicate chords per platform in `TERMINAL_KEYS`),
  `crates/gui/farhelm-gui/src/desktop/tests.rs` (`cmd-w` resolves to `DeleteSession`),
  `crates/gui/farhelm-gui/src/shortcuts/tests.rs` (shortcut table drift checks), `website/AGENTS.md` (when touching docs
  pages; note unattended runs start no preview server), `AGENTS.md` ("Changelog fragments", "Tests and validation"),
  `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`.
- **Planner-proposed implementation choices (reviewed by fresh-context planning review):**
  - Do not add `cmd-t`, `ctrl-shift-t`, or `ctrl-shift-w` to `TEXT_FIELD_KEYS`
    (`crates/gui/farhelm-gui/src/actions.rs`): all `TextInput`s live in focus layers already suppressed by
    `layers::focus_layer_open`.
  - Keep 2 actions for closing (`DeleteSession` on macOS `cmd-w` / Linux `ctrl-shift-backspace`, plus Linux-only
    `CloseTerminalTab` on `ctrl-shift-w`, mirroring `NewSession`'s single-platform row) and route `DeleteSession`
    through a `Platform`-parameterized helper in `MainView` so macOS tries closing the active shell tab first and falls
    through to `header::Action::Delete`, keeping `desktop/tests.rs` and existing `DeleteSession` tests clean and
    unit-testing macOS fallback on Linux. Update `shortcuts/tests.rs` to permit macOS's shared `⌘W` across
    `"Delete session"` and `"Close terminal tab"`.
  - Route `NewTerminalTab` and active-tab closing through `MainView` → `SessionPane` → `TerminalSet` (matching
    `PreviousTerminalTab`/`NextTerminalTab`, not through `session_pane/header.rs`), extracting a small private
    `focus_layer_open || terminal_is_composing` helper in `main_view.rs`.
  - Reuse `TerminalSet::open_tab` (guarded by `!self.inert && !delete::busy(&self.key, cx)`), retain each shell tab's
    `Anchor` on `TerminalSet` (e.g., `close_anchors: RefCell<HashMap<String, Anchor>>`), guard shortcut tab close by
    `!self.inert && !closing && !self.confirmed_closed.contains(&id)`, and call `TerminalSet::reveal_close(strip_index)`
    synchronously right before `ask_to_close` (no multi-frame visibility state machine needed, since `Anchor` updates
    bounds in ground prepaint before overlay prepaint).
  - Add `NEW_TERMINAL_TAB_SHORTCUT_NAME` (`"⌘T"` / `"Ctrl+Shift+T"`) in `actions.rs`, wire it into the `+` button
    tooltip (`terminal/set.rs`), and cross-check it in `shortcuts/tests.rs`.
  - In `crates/gui/farhelm-gui/src/session_list/delete.rs`, add `|| !target.row.info.tabs.is_empty()` to `delete::asks`
    (which automatically updates row-menu Delete, header Delete, shortcut Delete, and `session_list/menu.rs`'s
    `delete_shortcut` key chip). Also fix `session_pane/header.rs::hover_help` so the Delete tooltip does not promise
    immediate deletion when `Skip session confirmations` is on and shell tabs are open. Leave `session_list/bulk.rs`
    unchanged (outside single-session Delete scope; state cleanly in `SPEC.md`). Update
    `session_pane/header_tests.rs:3170` (`a_running_delete_in_a_narrow_pane_keeps_the_pill_whole_beside_many_tabs`),
    which currently deletes a 15-tab session with `Skip session confirmations` on.
  - Note: `terminal-tab-overflow-ux` also touches `crates/gui/farhelm-gui/src/terminal/set.rs`; if
    `terminal-tab-overflow-ux` has already landed by execution time, shell tab indices in `STRIP_TABS_ID` no longer
    include `Agent` at index 0 — verify `reveal_close`'s index against current `main`.

### PR shape (2 PRs)

1. **`plan/terminal-tab-shortcuts/01-warn-delete-with-open-tabs`:** Update `delete::asks` (`session_list/delete.rs`) and
   Delete tooltip (`session_pane/header.rs`) so open shell tabs always require Delete confirmation even when "Skip
   session confirmations" is enabled; update `SPEC.md`, `website/src/content/docs/docs/using/stop-restart-resume.mdx`,
   `settings.md`, and affected tests (`header_tests.rs` + new tests).
2. **`plan/terminal-tab-shortcuts/02-cmd-t-and-cmd-w-tab-shortcuts`:** Add `NewTerminalTab` (`cmd-t` / `ctrl-shift-t`)
   and Linux `CloseTerminalTab` (`ctrl-shift-w`) + macOS `cmd-w` active-tab close dispatch, retain tab close anchors +
   synchronous `reveal_close` before `ask_to_close`, update `shortcuts.rs`, `+` tooltip, `SPEC.md`,
   `keyboard-shortcuts.mdx`, `work-in-a-session.mdx`, tests, changelog fragment, and remove all three `TODO.md` entries.

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
`cargo xtask records path plan/terminal-tab-shortcuts` (`plans/.agents/outside-files.md`, "Files outside the
repository"). Its `log.md` is the working log. Invoke the `agent-resumeable` skill with that resolved path: if `log.md`
already exists, read it in full and resume where the previous holder left off (cross-checking against VCS state and open
`plan/terminal-tab-shortcuts/*` PRs on GitHub); if it does not exist, this is a fresh start.

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
via `realpath` at run time. Put the watchdog status file at `farhelm-plan-terminal-tab-shortcuts-watchdog.json` in the
parent directory of the executing checkout (derived per `scode-build-goal`, "Placing the files") — never inside the
`plan/terminal-tab-shortcuts` record directory.

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
`plan/terminal-tab-shortcuts/*` PRs when resuming, never another plan's PRs), set up per `plans/.agents/executing.md`,
"Executing one plan". Use bookmark names `plan/terminal-tab-shortcuts/<nn>-<short-name>`. Keep every PR a draft — never
mark a PR ready and never merge. Restructure existing PRs in the stack instead of stacking fixup commits on top,
including PRs created by an earlier run. Never edit anything under `plans/`.

Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments", and "Tests and validation", plus
`.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`. The last code PR removes all three
covered `TODO.md` entries.

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
removes all three covered `TODO.md` entries in the last code PR, and satisfies the latest `## Decisions` entry (if any).
Delivering the report is the executor's closing step (`plans/.agents/executing.md`), not part of the goal.

## Release note

**Cmd+T and Cmd+W now open and close individual terminal tabs like in a typical terminal emulator.** Pressing Cmd+W on
the main agent tab still deletes the session, and deleting a session with open terminal tabs now always asks for
confirmation.

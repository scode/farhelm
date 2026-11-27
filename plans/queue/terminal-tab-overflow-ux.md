# Fix overflowing terminal tabs

## Goal

Keep terminal tabs visible and easy to navigate when a session has more open shell tabs than fit horizontally beside the
right-hand header toolbar: pin the main `Agent` (or `Shell`) tab at the left, always auto-scroll the active shell tab
into view on create/switch/close/restore, and show subtle edge fades, `<`/`>` scroll buttons, and an all-tabs `▾`
dropdown menu whenever shell tabs overflow (keeping `| +` pinned at the right end of the strip).

Covers and removes this `TODO.md` entry in the last code PR:

- Immediate: **Overflowing terminal tabs go invisible.**

### Acceptance criteria

- The main `Agent` (or `Shell`) tab stays pinned at the left of the tab strip outside the scrolling shell-tabs container
  (with a narrow-strip fallback when the entire strip is squeezed below the 650 px + tunnel-pill threshold so the
  right-hand toolbar and tunnel pill remain whole per `SPEC.md`).
- Creating a new terminal tab, switching tabs (by click, keyboard shortcut, or dropdown), closing a tab (fallback
  selection), or restoring a remembered tab always scrolls the active shell tab into view, including on the frame a
  brand-new tab first enters layout.
- Subsequent manual scrolling (mouse wheel / trackpad / `<`/`>` buttons) is never fought by stale reveal state.
- When shell tabs overflow, subtle left/right edge fades appear on whichever edge has hidden tabs, `<` and `>` scroll
  buttons step the strip left/right (disabled at ends), and a compact `▾` all-tabs dropdown button (showing e.g. tab
  count + chevron) opens a popover listing every tab (`Agent` + all shell tabs) with an active indicator, one-click
  switching, and a close (`×`) button on each shell tab.
- Overflow controls appear only when shell tabs actually overflow (computed from layout-independent content width so
  mounting the buttons cannot oscillate or make them permanently sticky) and degrade gracefully in very tight strips
  (dropping `<`/`>` first while keeping `▾` so 1-click navigation stays available).
- Closing a shell tab from the `▾` popover closes the popover first and anchors any confirmation prompt cleanly to the
  `▾` toggle button (or centered fallback) rather than an unmounted popover element or off-screen tab.
- `| +` stays pinned at the right end of the tab strip.
- `SPEC.md`, `SPEC_impl.md`, unit/GPUI tests, and affected visual goldens are updated.

## Decisions already made

- **Overflow UX (maintainer decision):** Option A — auto-scroll active tab into view + subtle left/right edge fades +
  `<` / `>` scroll arrows + `▾` all-tabs dropdown menu (listing every tab with active marker and close action) when
  overflowing.
- **Pinning the main tab (maintainer decision):** Pin the main `Agent` tab at the left so it is always visible and one
  click away; only shell tabs scroll.
- **Review gate (maintainer decision):** Single fresh-context agent on **Opus 5.5** at **high** reasoning effort with
  the general review charter below.

## Implementation outline and requirement sources

### Requirement sources

- **User request and decisions:** Option A overflow controls + pinned `Agent` tab + auto-scroll active tab into view +
  settled release note below.
- **Binding repository constraints:** `SPEC.md` ("Session view", around lines 2544–2565, and the 650 px narrow-pane
  toolbar/tunnel-pill guarantee around lines 3698–3700), `SPEC_impl.md` (animations restricted to
  `components::transition`; no nested interactive controls inside controls), `AGENTS.md` ("Changelog fragments", "Tests
  and validation"), `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`.
- **Planner-proposed implementation choices (reviewed by fresh-context planning review):**
  - Leave `crates/gui/farhelm-gui/src/session_pane/header/fit.rs` (`STRIP_FLOOR_BESIDE_PILL = 112px`) untouched so the
    650 px + tunnel-pill guarantee and `header_tests.rs:2877` stay intact. Instead, extend `TerminalSet`'s existing
    `pins_add_at` width-tier check on `strip_bounds` (`crates/gui/farhelm-gui/src/terminal/set.rs`) into a small
    width-tier helper: full room pins `Agent`, `<`/`>`, `▾`, and `| +`; tight strip drops `<`/`>` first while keeping
    `▾`; extreme narrow strip (below `pins_add_at`, e.g., 650 px beside a tunnel pill) unpins `Agent` and `| +` into
    `STRIP_TABS_ID` and documents that narrow fallback in `SPEC.md` alongside the existing `+` fallback sentence.
  - Consolidate auto-scroll into **one** `pending_reveal_chosen` flag on `TerminalSet` armed at `set_choice` and
    `follow_tabs` restore, resolved once in `TerminalSet::strip` (`self.scroll.scroll_to_item(index)`), and remove
    `TerminalSet::step`'s duplicate strip-index scroll path. Reuse `STRIP_ID`'s existing `.on_children_prepainted` hook
    to trigger one deferred `window.refresh()` when a brand-new tab's layout bounds aren't populated yet.
  - Paint edge fades at paint time using an absolutely positioned `gpui::canvas` + `gpui::linear_gradient` overlay over
    `STRIP_TABS_ID` (following `crates/gui/farhelm-gui/src/settings/frame.rs`'s `page_rule` idiom locally without
    generalizing `page_rule`), fading from `palette.bg_1` to transparent based on `scroll.offset().x` and
    `scroll.max_offset().x`.
  - Compute overflow state inside `STRIP_ID`'s existing `on_children_prepainted` hook from layout-independent content
    width (`max_offset().x > eps` when hidden vs. `max_offset().x > controls_width + eps` when shown) so `<`/`>`/`▾`
    never stick or oscillate. Step `<`/`>` by a fixed fraction of the viewport width (clamped to `[-max_offset.x, 0]`),
    with no custom smooth-scroll animation or repeat timers.
  - Build the `▾` all-tabs popover by reusing `layers::open_popover`, a session-keyed `Anchor`, and `ask_to_close`
    (closing the popover first and anchoring `ask_to_close` to the `▾` toggle anchor). Do NOT generalize
    `components/menu_item.rs`; compose each shell-tab row locally as a flex row with a select button + sibling
    `IconButton` for `×` (matching `TunnelList` in `terminal/tunnels.rs`). Choose either `layers::panel` or
    `TunnelList`'s `layers::surface` + `.tab_through()` consistently and log as `DECISION`; for `<`/`>` icons, either
    rotate `Icon::ChevronDown` via `Button::new().square().content(...)` or add `ChevronLeft`/`ChevronRight` to
    `assets.rs` without adding a rotation method to `IconButton`.
  - Note: `terminal-tab-shortcuts` also touches `crates/gui/farhelm-gui/src/terminal/set.rs`; if
    `terminal-tab-shortcuts` has already landed by execution time, update `reveal_close`'s index for shell tabs (now
    0-based inside `STRIP_TABS_ID` when `Agent` is pinned outside).

### PR shape (2 PRs)

1. **`plan/terminal-tab-overflow-ux/01-pin-agent-and-autoscroll`:** Pin the main `Agent` tab outside `STRIP_TABS_ID`
   (with narrow-strip width-tier fallback integrated into `pins_add_at`) and unify chosen-tab auto-scroll on
   `TerminalSet` (removing `step`'s duplicate scroll code), handling new-tab single-frame reveal via
   `on_children_prepainted`.
2. **`plan/terminal-tab-overflow-ux/02-overflow-fades-arrows-and-menu`:** Add layout-independent overflow detection,
   paint-time `canvas` edge fades, `<`/`>` scroll buttons, and the `▾` all-tabs popover (reusing `layers` and
   `ask_to_close` anchored to `▾`), update `SPEC.md`, `SPEC_impl.md`, tests/goldens, changelog fragment, and remove the
   `TODO.md` entry.

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
`cargo xtask records path plan/terminal-tab-overflow-ux` (`plans/.agents/outside-files.md`, "Files outside the
repository"). Its `log.md` is the working log. Invoke the `agent-resumeable` skill with that resolved path: if `log.md`
already exists, read it in full and resume where the previous holder left off (cross-checking against VCS state and open
`plan/terminal-tab-overflow-ux/*` PRs on GitHub); if it does not exist, this is a fresh start.

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
via `realpath` at run time. Put the watchdog status file at `farhelm-plan-terminal-tab-overflow-ux-watchdog.json` in the
parent directory of the executing checkout (derived per `scode-build-goal`, "Placing the files") — never inside the
`plan/terminal-tab-overflow-ux` record directory.

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
`plan/terminal-tab-overflow-ux/*` PRs when resuming, never another plan's PRs), set up per `plans/.agents/executing.md`,
"Executing one plan". Use bookmark names `plan/terminal-tab-overflow-ux/<nn>-<short-name>`. Keep every PR a draft —
never mark a PR ready and never merge. Restructure existing PRs in the stack instead of stacking fixup commits on top,
including PRs created by an earlier run. Never edit anything under `plans/`.

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

**Overflowing terminal tabs now stay visible and easy to reach.** Opening or switching to a tab brings it into view
automatically, and overflow controls let you navigate all open tabs in a session.

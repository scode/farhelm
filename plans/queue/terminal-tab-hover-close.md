# Goal: Prevent accidental terminal tab closes

Move each shell tab's close button from the always-visible right side (next to the `+` new-terminal button) to a fixed
leading slot on the left inside the tab pill, showing a terminal icon at rest and swapping to a close icon on hover,
keyboard focus, open close confirmation, or in-flight close, with zero layout shift.

## Checkout and project identity

This goal runs inside a checkout of `scode/farhelm` ("the checkout the executing session runs in"). Confirm that the
nearest Git worktree or jj workspace containing the current working directory (a `.git` file counts as well as a `.git`
directory; never walk up to an enclosing checkout if the nearest fails) has a remote pointing at `scode/farhelm` (SSH or
HTTPS, with or without `.git`). If not, stop immediately before the resume protocol, write nothing (not even a log), and
report the mismatch. Every project path below is relative to that checkout's root.

## Resume protocol

Right after confirming the checkout, resolve the plan's record directory with
`cargo xtask records path plan/terminal-tab-hover-close` (`plans/.agents/outside-files.md`, "Files outside the
repository"), and invoke the `agent-resumeable` skill with `<record-dir>/log.md`. If that log already exists, read all
of it (and anything in `<record-dir>/notes/`) and resume where the previous run left off, cross-checking the log against
reality (VCS state and open `plan/terminal-tab-hover-close/*` PRs on GitHub, with GitHub authoritative for what was
pushed). If the log does not exist yet, this is a fresh start.

## Execution mode

No-workhorse mode. Do not send any unit of your own decomposition to a weaker model or lower reasoning effort than your
own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point. If
`scode-galaxy-brain` is already active when this run starts, this goal file is the maintainer's express instruction to
stop it.

Same-model sub-agents at your own model and reasoning effort are fine when they are likely to finish work sooner without
costing quality (at your judgment; concurrent writers may use separate worktrees or jj workspaces whose results you
merge back, while you still own the `jjstack` stack and every PR). Launch sub-agents natively when available, or through
`scode-harness-shellout` when the demanded model is not available as a native sub-agent type.

The sub-agents listed in `plans/AGENTS.md`, "Sub-agents this file requires" (resume check, cold reads of Blocked
questions and reports), the per-PR review gate below, and the scope reassessment review below are exempt from any
no-delegation constraint and run as specified.

## Resource watchdog

Right after the resume protocol and before any delegation, start the `scode-build-goal` resource watchdog so memory and
disk are monitored for the rest of the run:

- **Script path:** resolve `scripts/watchdog` next to the `scode-build-goal` skill's `SKILL.md` with symlinks resolved
  (`realpath`).
- **Status file:** `farhelm-plan-terminal-tab-hover-close-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files"), never inside the `plan/terminal-tab-hover-close`
  record directory.
- **Watched paths (`--path`):** the root of the executing checkout, the home directory, `/tmp`, the parent directory
  holding the status file, the session's own scratch directory, and any worktree or build directory created on another
  filesystem (restart the sampler with it added). Interval: 60 seconds, with the script's default alert thresholds.
- **Delivery and verification:** follow `scode-build-goal`, "Resource watchdog" for the active harness (Claude Code dual
  Monitors, Codex `skillette-codex-wake-on-exit`, or the plain background sampler fallback when neither exists). Prove
  the mechanism once on throwaway status files in your scratch directory (`--interval 5`, `--synthetic-alert`, kill
  test, `--stall-after 2`) before arming the real monitors, and stop the throwaway runs afterwards.
- **Acting and lifecycle:** record handles, paths, and mechanism in the working log. Run
  `watchdog check --status <file>` before every new workload launch and at natural boundaries. On `ALERT`, `WORSE`, or a
  failed `check`, pause launches and free run-owned space before resuming. On `STALE`, `ERROR`, or monitor exit, pause
  launches until monitoring is restored and `check` passes. Reconcile any still-fresh earlier session's sampler on
  resume before starting yours, and stop your owned monitors when the run ends.

## Goal and acceptance criteria

Make it much less likely that a user accidentally clicks a shell tab's close button when intending to click `+` to open
a new terminal tab.

Acceptance criteria:

1. Each shell tab (`Terminal N`) renders a fixed-size leading slot on the **left** inside its tab pill:
   - At rest, the slot shows `Icon::Terminal`.
   - When the tab is hovered (using `HoverListenerMode::InputModalityIndependent`), when keyboard focus is on the tab or
     its close button, while that tab's close confirmation prompt is open, or while that tab's close request is in
     flight, the leading slot shows `Icon::Close` (`×`) instead of `Icon::Terminal`.
   - Hovering or revealing the close button causes **zero layout shift**: tab widths, neighboring tabs, and `+` do not
     move.
   - Hovering the close button on an unselected tab keeps the pill's hover background continuous across both the close
     button and the label, and hovering the close button itself shows clear close hover styling (with readable contrast
     both on an unselected tab and on a selected accent-filled tab).
2. The main `Agent` / `Shell` tab has no leading icon and no close button (unchanged).
3. Clicking the close button keeps existing confirmation behavior: it respects **Skip session confirmations** in
   Settings, and when confirmations are enabled it opens the anchored `"Close Terminal N?"` confirmation prompt under
   the close button (with focus on `Confirm close`, and the close button remaining visible while the prompt stays open
   even when the pointer moves onto the prompt).
4. Keyboard accessibility and narrow-strip behavior remain intact:
   - Tab order stays `Agent -> Terminal 1 -> Close Terminal 1 -> Terminal 2 -> Close Terminal 2 -> +`, with `Close`
     always keyboard-reachable via `Tab` even before hovering.
   - At a 650 px narrowed strip, focusing either `Terminal N` or `Close Terminal N` reveals it in view.
   - macOS title-band drag in gaps between tabs remains intact.
5. `SPEC.md` ("Session view") documents the left-side hover-revealed terminal tab close button concisely.
6. Unit tests, header narrow-strip tests, desktop title-band drag tests, and affected screenshot goldens
   (`session-toolbar`, `terminal-cut-session-row`) are updated and pass.
7. The `TODO.md` entry **"Stop accidental terminal tab closes when creating a new terminal."** is removed in the last
   code PR.

## Decisions already made

### Requirement source 1: User request and settled decisions

- **Original TODO entry:** _"Stop accidental terminal tab closes when creating a new terminal. Find some way of making
  it much less likely that a human will accidentally click the close button for a terminal tab when they intended to
  create a new terminal. I keep doing this all the time."_
- **Maintainer picker note:** Preferred Ghostty-style left-side close on hover so no close button sits next to `+`, open
  to variations.
- **Settled layout (Design A):** Fixed leading terminal icon on the left inside each shell tab pill that swaps to `×` on
  hover, with zero layout shift so neighboring tabs and `+` never move.
- **Settled reveal rule:** Reveal `×` on any shell tab on hover or keyboard focus (not restricted to the active shell
  tab), plus while its close confirmation prompt is open or close is in flight.
- **Settled confirmation & shortcuts:** Keep existing confirmation behavior (respect **Skip session confirmations**,
  anchored prompt when enabled). Do not add a new close-tab keyboard shortcut.
- **Settled review gate:** Fresh-context **Opus 5.5** agent at **high** effort with the general review charter.
- **Settled release note:** Recorded verbatim in `## Release note` below.

### Requirement source 2: Binding repository constraints

- `SPEC.md` ("Session view", "Terminal experience", "Opening and closing"): shell tabs close individually; **Skip
  session confirmations** skips tab close; close confirmation starts with focus on the confirming button; top row stays
  keyboard-reachable down to a 650 px pane without wrapping.
- `SPEC_impl.md` and `crates/gui/clippy.toml`: GPUI animation methods (`with_animation`, `with_spring`, `MotionReveal`,
  `request_animation_frame`) are forbidden outside `components/transition.rs` and `ticker.rs`; instant reveal is used
  here (no animation needed).
- `crates/gui/farhelm-gui/src/components/copy_field.rs` (hover-layout invariant): never use `.group_hover(...)` to
  change layout or mount/unmount elements (breaks GPUI's prepaint/paint frame).
- `crates/gui/farhelm-gui/src/components/mark_toggle.rs` and `tab_strip.rs`: do not reuse `MarkToggle` (non-focusable)
  or `TabStrip` (dialog `role="tablist"`).
- `docs/harness-marks.md`: do not use `Icon::HarnessTerminal` (a 12-unit harness mark for agent identity); use standard
  14-unit action icons `Icon::Terminal` and `Icon::Close` (`crates/gui/farhelm-gui/src/assets.rs`).
- `AGENTS.md` ("Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation"),
  `.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`.

### Requirement source 3: Planner-proposed implementation choices (scope-reviewed)

These choices came from repository inspection and a fresh-context planning scope review; preserve them unless reality in
the checkout requires a documented `DECISION` or scope reassessment:

1. **Reuse `Tier::TerminalTab` unchanged and adapt `Tier::TerminalTabClose` in place
   (`crates/gui/farhelm-gui/src/components/button.rs`):**
   - Use existing `Button::content` on `Tier::TerminalTab` to render
     `[fixed leading Icon::Terminal slot + "Terminal N"]` with zero new `Tier` variants or custom shell-tab widgets.
   - Adapt the single-caller `Tier::TerminalTabClose` in place to `Tier::TerminalTabClose { on_fill: bool }` (mirroring
     existing `Tier::RowMenu { on_fill: bool }`) so `Icon::Close` has proper contrast and hover/focus styling both on an
     unselected tab and on a selected `accent_fill` blue pill.
2. **Always-mounted overlaid sibling for `Close` (`crates/gui/farhelm-gui/src/terminal/set.rs`):**
   - Make each shell tab's existing wrapper `div()` `.relative().rounded(metrics::RADIUS_SM)` with
     `.hover_listener_mode(HoverListenerMode::InputModalityIndependent)` and a local deduped `window.use_keyed_state`
     hover `Entity<bool>` (checking `if *state != hovered` before notifying; kept local to `terminal/set.rs` rather than
     modifying `session_list/` for only two callers).
   - Render Child 1 (`tab`, `Tier::TerminalTab` with leading `Icon::Terminal` at `opacity(0.)` when revealed and
     `opacity(1.)` at rest) and Child 2 (an absolutely-positioned left-slot wrapper holding the always-mounted
     `IconButton` using `Icon::Close` + `Tier::TerminalTabClose { on_fill: selected }` and `anchor.track()`, at
     `opacity(1.)` when revealed and `opacity(0.)` at rest, with a `:shown` / `:hidden` debug selector matching
     `session_list/menu.rs`).
   - Add `.when(hovered && !selected && !inert, |pill| pill.bg(theme::color(palette.control_hover)))` on the
     `.relative()` wrapper so pointing at the sibling `Close` button preserves an unselected tab's hover background
     seamlessly.
   - This avoids nested interactive click propagation, preserves macOS title-band drag gaps (`desktop/tests.rs`), keeps
     `Tab` order `Terminal N -> Close Terminal N`, and keeps `Close` + `anchor.track()` mounted at all times.
3. **Single `asking: Option<(String, LayerId)>` on `TerminalSet`:**
   - Capture the `Option<LayerId>` already returned by `prompt::ask_anchored` in `ask_to_close` (mirroring
     `session_pane/header.rs`'s `asked: Option<(Control, LayerId)>`, not `session_list/menu.rs`'s `RowLayers` map).
     Check `layers::is_open(*layer, window, cx)` in `TerminalSet::strip` so `Close` stays visually revealed while the
     confirmation prompt is open.
   - Reveal predicate:
     `hovered || tab_focus.is_focused(window) || close_focus.is_focused(window) || asking_this_tab || closing` (gated by
     `!inert` for hover, while still keeping closing/asking states honest).
4. **Delete `TerminalSet::reveal_end`:**
   - `reveal_end` existed only to reveal a right-side `×` in a narrowed 650 px strip; with `×` on the left, keeping it
     would scroll `×` out of view. Delete `reveal_end` and route both `StripControl::Tab(n)` and
     `StripControl::Close(n)` to `self.scroll.scroll_to_item(n + 1)`.
5. **Single PR:** Keep the production changes, unit/header/drag test updates (including updating `strip_closes()` and
   `the_agent_terminals_tab_has_no_close_control` in `terminal/set/tests.rs` from text `"×"` / right-edge assertions to
   icon/left-slot + reveal assertions), golden refreshes (`session-toolbar`, `terminal-cut-session-row`), `SPEC.md`
   update, changelog fragment, and `TODO.md` removal in **one single PR** so golden tests never fail between PRs.

## Scope reassessment

Before implementing a substantial departure from the outline above (for example, introducing a new widget abstraction,
animation helper, or generalizing `session_list/` hover/layer structs), or after repeated corrective reviews of the same
component, run a fresh-context review at your own model and effort using this exact charter:

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

Supply the original request, settled decisions, outline, current diff, and proposed departure (what changed, why it is
necessary, and which simpler alternative was ruled out). A close reuse-versus-duplication call that stays within agreed
scope is a routine fork: decide it after reassessment and log it as `DECISION`. If a sub-agent discovers an unapproved
new mechanism mid-unit, leave it unimplemented and report it rather than inventing it to satisfy a checkpoint.

## PR discipline and validation

- Base the stack on `main@origin` (or on this plan's own open `plan/terminal-tab-hover-close/*` PRs when resuming, never
  another plan's PRs), set up per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmark naming `plan/terminal-tab-hover-close/<nn>-<short-name>` (1 PR expected). PRs stay drafts;
  never mark a PR ready and never merge. Restructure an existing open PR of this plan rather than stacking a fixup PR on
  top.
- Never edit `plans/`.
- Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments" (add a fragment under `releasing/changelog.d/` in
  the same commit), and "Tests and validation", plus `.agents/validation.md`, `.agents/test-evidence.md`, and
  `.agents/test-authoring.md`. Remove the covered `TODO.md` entry in the last code PR.

## Review gate

Before finishing any PR in this plan, delegate a review of its changes to a single fresh-context **Opus 5.5** agent at
**high** reasoning effort (explicit demand for this model and effort; launch natively or via `scode-harness-shellout` —
do not invent a custom shell command).

Give the reviewer the checkout root (resolved at run time), the commit range or bookmark to review, the private findings
file path in your scratch directory, `.agents/test-authoring.md` verbatim whenever tests or fixtures changed, and this
complete charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved. Report findings to the named findings file, most
> severe first, with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no VCS
> state.

Address every valid finding and re-verify before moving on.

## Decision logging

Log major design decisions in the plan's working log as they happen with a scannable `DECISION` label, including the
alternatives considered.

## Unattended fallback

This plan executes unattended. Never ask the maintainer anything in conversation — not a direct question, not a harness
question prompt, not an optional offer or closing "should I...?" (`plans/AGENTS.md`, "Unattended means no questions").

- Resolve routine implementation forks within the agreed scope and log consequential choices as `DECISION`. Remove
  unnecessary planner-invented machinery when the requested behavior and repository constraints still hold.
- When remaining work requires a decision only the maintainer can make (a material scope expansion, weakened guarantee,
  or omitted required behavior not covered by settled decisions), push everything consistent and block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking).
- Put things the maintainer should know and optional follow-up ideas into the closing report rather than stopping or
  asking.

## Done criterion

The goal is achieved when every PR in the stack exists as a draft PR on GitHub, passes its review gate and chosen
validation, satisfies the acceptance criteria above (including `TODO.md` entry removal), and satisfies the latest
`## Decisions` entry in this file, if any. Delivering the report is the executor's closing step, not part of the goal.

## Release note

Terminal tab close buttons now appear on the left on hover. Previously there was a close button to the right (always
visible), which was easy to accidentally click instead of the + button used to open a new terminal tab.

## Decisions

### 2026-11-27: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-11-27 (claim c5fb4b).
>
> ### Blocked at landing: a second New terminal press now lands on the new tab's close button
>
> **The feature and the original problem.** In an open session's top row, each shell tab ("Terminal 1", "Terminal 2",
> ...) had a × on its right, and the last tab's × sat right next to `+` (New terminal). You kept closing a terminal tab
> by accident when you meant to open a new one.
>
> **What you decided.** A Ghostty-style tab: each shell tab gets a fixed slot on its left that shows a terminal icon at
> rest and turns into the × when the pointer is over the tab, keyboard focus is on the tab or its ×, its close question
> is open, or its close is under way, with nothing in the row moving when it does. Closing otherwise behaves as before.
>
> **What the plan built.** Exactly that, and it is reviewed and its checks pass (PR #5096, not yet merged).
>
> **What the landing found.** `+` still sits right after the last tab, so pressing it adds a tab where `+` was and moves
> `+` one tab to the right. The new tab's left slot, and so its ×, lands within a pixel of where `+` was (measured in
> the running app). Because the pointer is over that tab, the × also shows, in red. So pressing `+` twice without moving
> the pointer, a natural way to open two terminals, presses the new tab's × the second time. With confirmations on, the
> new tab's close question ("Close Terminal 2?" when Terminal 1 was already open) opens with Confirm close focused, so a
> reflexive Enter ends the shell it just opened. With Skip session confirmations on, that shell ends at once. Before
> this change, the same second press landed on the new tab's name, which was already showing, so it did nothing. This is
> the accident the plan set out to remove, in a new form. Your decision that nothing moves was about hovering; it does
> not say where `+` goes once a tab is added, so this needs a choice.
>
> **The question: how should the second press be kept off the new tab's close button?**
>
> - **(a) Pin `+` at the far right of the tab area.** `+` stays at the right edge of the space the tabs can use, just
>   left of the desktop tunnel pill, so adding a tab never moves it. New tabs fill in from the left, and a new tab's ×
>   is never under the pointer. With few tabs open, `+` sits well away from them, with empty space between, so it is a
>   longer reach and has the tunnel pill as a new neighbor. This matches Ghostty. It changes the header's layout and two
>   or three reference screenshots.
> - **(b) Put `+` to the left of the shell tabs**, between the Agent tab and Terminal 1. It stays fixed and close to the
>   tabs, and new tabs appear away from the pointer. But it puts `+` right beside the first shell tab's × slot, a tab's
>   padding away: the same neighbors as the accident you first reported, only with the × hidden until hover. Not what
>   Ghostty does.
> - **(c) Disarm a new tab's × until the pointer leaves it.** Keep the layout; a newly opened tab does not show or
>   accept presses on its × until the pointer has left that tab once, so a second press does nothing, as before. A
>   smaller change, but a hidden rule that applies only right after opening a tab, and the × still ends up under the
>   pointer.
> - **(d) Land as is** and live with the double press. Not recommended: it brings back the accident in a new form.
>
> **Recommendation: (a).** It removes the cause instead of guarding against it, keeps one simple rule (`+` never moves),
> keeps `+` away from every ×, and is what the Ghostty reference does. If you choose (a), the plan goes back to an
> executor to pin `+` in the same PR, keeping the rest of the work as built.

The maintainer's answer:

### Decision (landing monitor, on the maintainer's behalf)

The landing monitor answered this block on the maintainer's behalf, under the standing rule that a block that is not a
huge question (it drops no requested feature, risks no data, and is easily reversed) gets a decision rather than a wait.
The maintainer can change it later; a change of mind is a follow-up, not a reason to stop the plan.

**Chosen: (a) Pin `+` at the far right of the tab area.** `+` (New terminal) stays at the right edge of the space the
tabs can use, just left of the desktop tunnel pill, so adding a tab never moves it and a new tab's × never ends up under
the pointer. New tabs keep filling in from the left. Everything else the plan built stays as it is: the left slot with
its terminal icon and hover-revealed ×, the keyboard order (each tab, then its ×, then the next tab, then `+` last), and
the close question's behavior.

What to do, in the same PR (#5096), not a new one:

- Move `+` out of the scrolling row of tabs so it stays at the tab area's right end however many tabs are open, and
  stays visible when the tabs scroll.
- Keep `+` last in the keyboard order, and keep a focused tab or × scrolled into view as now.
- Add a test that pins the reported case: pressing `+` and then pressing again at the same spot opens a second terminal
  and closes nothing, with Skip session confirmations both on and off.
- Update SPEC.md's "Session view" to say `+` stays at the right end of the tab area, re-take the reference screenshots
  this changes, and look at them in both appearances.
- Note the change in the report.

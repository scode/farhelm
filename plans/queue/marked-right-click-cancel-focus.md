# Plan: Return focus to pre-right-click target when cancelling marked context-menu dialogs

## Goal

Fix keyboard focus return when cancelling a bulk **Stop** or **Delete** confirmation — or a **Move to group → New
group…** dialog — opened from a marked session's right-click menu so focus returns to where it was before the
right-click (usually the terminal) instead of jumping to the marked-sessions bar's menu (`⋯`) button.

Acceptance criteria:

- Cancelling (Cancel, Escape, or Close) a bulk **Stop** or **Delete** confirmation opened from the marked right-click
  menu returns keyboard focus to the pre-right-click focus target (usually the terminal), or recovers to home if that
  target vanished.
- Cancelling a bulk **Stop** or **Delete** confirmation opened from the marked-sessions bar's Stop/Delete buttons
  continues returning focus to the marked bar's `⋯` toggle (or the triggering button if `⋯` is not drawn), and
  confirming either way still focuses the marked count per `SPEC.md`.
- Cancelling **Move to group → New group…** opened from the marked right-click menu returns focus to the pre-right-click
  target, while opening it from the marked bar's `⋯` menu continues returning focus to the bar's `⋯` toggle.
- Outdated code comments claiming marked context-menu confirmations or group dialogs return to the bar's `⋯` button are
  updated.
- Regression tests in `crates/gui/farhelm-gui/src/session_list/context_menu_tests.rs` cover cancelling Stop/Delete
  confirmations from the marked right-click menu AND cancelling **Move to group → New group…** from both the marked
  right-click menu and the marked bar's `⋯` menu (including under macOS click rules).
- The existing unreleased changelog fragment `releasing/changelog.d/session-right-click-menu.md` (from PR #5090) is
  amended per `AGENTS.md`, "Changelog fragments".
- The last code PR removes the
  `Return keyboard focus to the pre-right-click target when cancelling a Stop or Delete confirmation from a marked right-click menu.`
  entry from `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** From PR #5090: picking Stop or Delete from a marked session's right-click menu and
  pressing Cancel currently jumps focus to the marked-sessions bar's menu button instead of returning to the
  pre-right-click target (usually the terminal). Change Cancel on bulk confirmations opened from a marked right-click
  menu to return focus to the pre-right-click target.
- **Settled maintainer decisions:**
  - Fix cancelling Stop and Delete bulk confirmations opened from the marked right-click menu so focus returns to the
    pre-right-click target (keeping bar clicks returning to the bar's `⋯` button and Confirm focusing the marked count).
  - Also fix cancelling **Move to group → New group…** opened from the marked right-click menu so it returns focus to
    the pre-right-click target instead of the bar's `⋯` button.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `SPEC.md` (focus return rules and bulk Stop/Delete), `AGENTS.md` ("Conventional
  Commits", "Changelog fragments", "Tests and validation"), and `plans/AGENTS.md`.
- **Planner-proposed choices (from fresh-context planning review):**
  - Do **not** thread caller origin into `apply_metadata` (`crates/gui/farhelm-gui/src/session_list/marking.rs`):
    `apply_metadata` already calls `self.close_marked_menu(window, cx)` first, and `layers::close` synchronously
    restores focus to `menu_focus` (for the bar's `⋯` menu) or the pre-right-click target (for the right-click menu).
    Simply changing `marking.rs:1066` to `focus::ReturnTo::Opener` fixes both without any extra parameter, matching
    `SessionList::group_name_from_menu` in `menu.rs`.
  - For `bulk_lifecycle` / `ask_bulk` (which is called directly from the bar's Stop/Delete buttons with no menu closing
    beforehand), pass `focus::ReturnTo` (or `Option<FocusHandle>`) rather than inventing a new `Bar | ContextMenu` enum.
  - Update code comments in `marking.rs` and `bulk.rs` rather than adding redundant `SPEC_impl.md` prose (`SPEC_impl.md`
    does not currently state the old marked-context-menu-returns-to-bar rule).
  - Single PR (`plan/marked-right-click-cancel-focus/01-restore-right-click-cancel-focus`).

## Implementation outline

Single small PR in `crates/gui/farhelm-gui/src/session_list/`:

1. In `crates/gui/farhelm-gui/src/session_list/bulk.rs` and `marking.rs`:
   - Pass `returns_to: focus::ReturnTo` (or `Option<FocusHandle>`) into `bulk_lifecycle` and `ask_bulk`.
   - From the marked bar's Stop/Delete buttons (`marking.rs`), pass the existing
     `Control(menu_toggle_drawn().unwrap_or(button_focus(action)))` target.
   - From `MarkedMenu::lifecycle_items` (`marking.rs`, immediately after `list.close_marked_menu(window, cx)` which has
     synchronously restored focus to the pre-right-click target or `menu_focus`), pass `focus::ReturnTo::Opener`
     (omitting `.returns_to(...)` on `Question` so it uses its default `ReturnTo::Opener`, capturing the restored
     pre-right-click target for right-click or `menu_focus` for bar `⋯`). Confirm `reword_bulk_question` preserves the
     layer's `returns_to`.
2. In `crates/gui/farhelm-gui/src/session_list/marking.rs` (`SessionList::apply_metadata`):
   - Change `rename::group`'s `returns_to` argument from `focus::ReturnTo::Control(self.marking.menu_focus.clone())` to
     `focus::ReturnTo::Opener`, relying on `self.close_marked_menu(window, cx)` having synchronously restored focus to
     either the bar's `⋯` button or the pre-right-click target.
3. Update the stale comments in `marking.rs` and `bulk.rs` (checking `SPEC_impl.md` too in case any sentence needs
   updating).
4. Add regression tests in `crates/gui/farhelm-gui/src/session_list/context_menu_tests.rs` covering:
   - Cancelling bulk Stop and Delete confirmations opened from the marked right-click menu returns focus to the
     pre-right-click target (usually the terminal), including when the focused target vanishes while the modal is open
     (recovers to home).
   - Confirming still focuses `"N marked"`.
   - Cancelling **Move to group → New group…** returns focus to the pre-right-click target when opened from the marked
     right-click menu and to the bar's `⋯` button when opened from the bar's `⋯` menu (including under
     `CLICK_FOCUSES_CONTROLS = false`).
5. Amend `releasing/changelog.d/session-right-click-menu.md` and remove the covered entry from `TODO.md`.

## Scope reassessment

Before implementing a substantial departure from the outline — or after repeated corrective reviews of the same
component — run a fresh-context review at the executing agent's own model and effort with this charter:

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

Supply the request, settled decisions, outline, current diff, and proposed departure (what changed, why it is necessary,
and which simpler alternative was ruled out). A routine fork within the agreed scope is decided after reassessment and
logged as `DECISION`. Any material scope expansion, weakened guarantee, or omitted required behavior follows the
unattended fallback below.

## Checkout and resume protocol

1. **Checkout check:** Run inside the checkout the executing session runs in, which must be the nearest Git worktree or
   jj workspace containing the working directory (a `.git` file counts as well as `.git`). Confirm a remote points to
   `scode/farhelm` (compared as host, owner, and name). If not, stop before the resume protocol, write nothing, and
   report the mismatch. Every project path in this plan is relative to that checkout root.
2. **Resume protocol:** Resolve the plan's working log directory with
   `cargo xtask records path plan/marked-right-click-cancel-focus` (`plans/.agents/outside-files.md`, "Files outside the
   repository"). Immediately invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already
   exists, read all of it and resume from where the previous session left off, cross-checking against GitHub and VCS
   state (`plans/.agents/executing.md`, "Executing one plan", step 7); otherwise start fresh.

## Execution mode and resource watchdog

- **No-workhorse mode:** This is an explicit no-workhorse choice. Never send any unit of your own decomposition to a
  weaker model or lower effort than your own, and never invoke, load, or follow `scode-galaxy-brain` or
  `scode-model-routing` at any point (if `scode-galaxy-brain` is already active, stop it). Same-model, same-effort
  sub-agents are allowed at your judgment when likely to finish sooner without costing quality (using separate worktrees
  or jj workspaces if concurrent writers would conflict, while you retain ownership of `jjstack` and every PR). The
  sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires", the per-PR review gate below, and any scope
  reassessment review are exempt from any no-delegation rule. Launch sub-agents natively where supported, or through
  `$scode-harness-shellout` when the demanded model is unavailable natively.
- **Resource watchdog:** Right after the resume protocol and before any delegation, start the `scode-build-goal` skill's
  `scripts/watchdog` (resolved with `realpath` at run time) using
  `farhelm-plan-marked-right-click-cancel-focus-watchdog.json` in the parent directory of the executing checkout (never
  inside the `plan/marked-right-click-cancel-focus` record directory). Watch the executing checkout root, `$HOME`,
  `/tmp`, the parent directory holding the status file, your scratch directory, and any worktree or build directory on
  another filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What the goal file
  must contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at boundaries,
  acting on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/marked-right-click-cancel-focus/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/marked-right-click-cancel-focus/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the
  docstring/comment rules with `scode-voice`. Run targeted `farhelm-gui` `session_list` tests through
  `python3 scripts/record-test-run.py`, plus `cargo fmt`, `cargo clippy`, `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py`.
- **Review gate (explicit demand):** Before finishing any PR, run a fresh-context review on **Opus 5.5** at **high**
  effort (natively if available, otherwise via `$scode-harness-shellout`; do not hardcode a launch command) with this
  charter: review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
  against the goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and
  whether a simpler shape would), and idiomatic code for the languages involved; report findings to a named private file
  in your scratch directory, most severe first, with the file, the quoted code, what is wrong, and the smallest fix;
  edit nothing and touch no VCS state. Because this PR changes Rust tests, also supply `.agents/test-authoring.md`
  verbatim to the reviewer. Address findings before moving on.

## Decision logging and unattended fallback

- Log consequential implementation choices in `log.md` as they happen with a scannable `DECISION` label and the
  alternatives considered.
- Never ask the maintainer anything in conversation — no direct questions, no harness question prompts, no optional
  next-step offers, and no waiting for confirmation (`plans/AGENTS.md`, "Unattended means no questions"). Decide routine
  implementation forks within the agreed scope, log them as `DECISION`, and carry on. Remove unnecessary
  planner-invented machinery when requested behavior and repository constraints still hold. If a material scope
  expansion, weakened guarantee, omitted required behavior, or decision only the maintainer can make comes up, block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Put things the maintainer should know and
  possible follow-ups in the report.

## Done criterion

Every PR in the stack exists as a draft PR on GitHub, satisfies the acceptance criteria (including removing the
`TODO.md` entry in the last code PR), passes its chosen validation and its Opus 5.5 high review gate, and satisfies the
latest `## Decisions` entry, if any. Delivering the report is the executor's closing step (`plans/.agents/executing.md`,
"Executing one plan", step 11), not part of this goal.

## Release note

No release note needed.

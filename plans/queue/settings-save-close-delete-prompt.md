# Plan: Align Settings Save-and-Close and Recipe leave prompts with inline Delete confirmations

## Goal

Codify in `SPEC.md` that choosing **Save** in Settings' unsaved-changes close or leave prompt dismisses an unanswered
inline **Delete** confirmation (covering both Custom agents and Recipes), and align Recipes' Back/Cancel leave prompt so
it preserves an open inline Delete confirmation when the user chooses **Keep editing** and dismisses it only when
**Save** starts.

Acceptance criteria:

- `SPEC.md` ("Opening and closing", Settings closing rules, and Recipes) carries an explicit exception allowing choosing
  **Save** in an unsaved-changes leave/close prompt to dismiss an unanswered inline Delete confirmation for that form,
  scoped so it does not contradict the rule that a newer dialog or menu opened from Settings stays open after saves
  finish.
- In `crates/gui/farhelm-gui/src/settings/recipes/editor/leaving.rs`, `Editor::ask_leave` no longer clears
  `self.delete_question` when the leave prompt opens; instead it relies on `Editor::started`
  (`crates/gui/farhelm-gui/src/settings/recipes/editor.rs`) clearing `self.delete_question` when **Save** starts, so
  **Keep editing** keeps the inline Delete confirmation open (matching Settings close and Custom agents).
- Module/doc comments in `leaving.rs` and `crates/gui/farhelm-gui/src/settings/custom_agents.rs` stay consistent with
  the aligned behavior.
- Unit tests in `crates/gui/farhelm-gui/src/settings/recipe_leave_tests.rs` verify that **Keep editing** on Recipes'
  Back/Cancel leave prompt preserves an open inline Delete confirmation while **Save** dismisses it.
- The last code PR removes the
  `Decide whether Settings "Save changes before closing?" should dismiss an open custom-agent Delete confirmation (and align SPEC.md).`
  entry from `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** Decide whether choosing Save in Settings' "Save changes before closing?" prompt
  should keep dismissing an unanswered custom-agent Delete confirmation (and add an explicit exception to `SPEC.md`), or
  change it back so Save-and-Close keeps the Delete confirmation open.
- **Settled maintainer decisions:**
  - Keep Save-and-Close dismissing an unanswered inline Delete confirmation and add an explicit exception to `SPEC.md`
    covering both Custom agents and Recipes.
  - Align Recipes' Back/Cancel ("Save these changes before leaving?") so **Keep editing** preserves the inline Delete
    confirmation and only **Save** dismisses it, matching Settings close and Custom agents.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `SPEC.md` ("Opening and closing", "Settings", "Recipes"), `AGENTS.md`
  ("Conventional Commits", "Changelog fragments", "Tests and validation"), and `plans/AGENTS.md`.
- **Planner-proposed choices (from fresh-context planning review):**
  - Remove the 5-line `delete_question = None` divergence in `leaving.rs` and reuse `Editor::started`'s existing
    `self.delete_question = None` in `recipes/editor.rs`.
  - Single PR (`plan/settings-save-close-delete-prompt/01-align-delete-prompt-on-save`).

## Implementation outline

Single small PR that removes a divergence and aligns `SPEC.md`:

1. Update `SPEC.md`:
   - Under `### Opening and closing` (the "Nothing closes a dialog, menu, popup list or confirmation that it did not
     open" rule), add a narrowly scoped exception for choosing **Save** in an unsaved-changes Settings close or editor
     leave prompt dismissing an unanswered inline Delete confirmation on that form.
   - Align the Settings closing and Recipes paragraphs so the exception is clear and does not conflict with the rule
     keeping a newer dialog or menu opened from Settings open after saves finish.
2. In `crates/gui/farhelm-gui/src/settings/recipes/editor/leaving.rs`:
   - Remove the early `self.delete_question = None` clearing inside `Editor::ask_leave` (and any now-unused `layer`
     binding). Saving via the leave prompt already flows through `Editor::started` (`recipes/editor.rs`), which clears
     `self.delete_question = None` once Save starts.
   - Update the comment at the top of `leaving.rs` and the cross-reference comment in
     `crates/gui/farhelm-gui/src/settings/custom_agents.rs`.
3. Update/add tests in `crates/gui/farhelm-gui/src/settings/recipe_leave_tests.rs` so **Keep editing** on Recipes' leave
   prompt preserves the unanswered inline Delete confirmation while **Save** dismisses it (keeping existing
   Settings-close coverage intact).
4. Amend `releasing/changelog.d/custom-agent-delete-question.md` (or leave a `kind: none` fragment as appropriate under
   `AGENTS.md`, "Changelog fragments"), and remove the covered entry from `TODO.md`. (Note: do not touch unrelated
   pre-#5075 fragments unless clearly required; flag any stale fragment in the report if observed.)

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
   `cargo xtask records path plan/settings-save-close-delete-prompt` (`plans/.agents/outside-files.md`, "Files outside
   the repository"). Immediately invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already
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
  `farhelm-plan-settings-save-close-delete-prompt-watchdog.json` in the parent directory of the executing checkout
  (never inside the `plan/settings-save-close-delete-prompt` record directory). Watch the executing checkout root,
  `$HOME`, `/tmp`, the parent directory holding the status file, your scratch directory, and any worktree or build
  directory on another filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What
  the goal file must contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at
  boundaries, acting on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/settings-save-close-delete-prompt/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/settings-save-close-delete-prompt/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the
  docstring/comment rules with `scode-voice`. Run targeted `farhelm-gui` Settings tests through
  `python3 scripts/record-test-run.py`, `cargo fmt`, `cargo clippy`, `dprint check`,
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

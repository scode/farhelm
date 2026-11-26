# Goal: Harden Settings Close, Save, Credentials, and Footer actions

Address six intertwined Settings close/save/footer follow-ups in one ordered stack on
`crates/gui/farhelm-gui/src/settings/` so they do not conflict with each other and follow the maintainer's narrowed
scope decisions.

**Source TODO entries (all six removed in the last code PR):**

- `Name each custom-agent draft when closing Settings.` (`## Plan follow-ups`)
- `Keep credential delivery reports visible when Settings closes.` (`## Plan follow-ups`)
- `Wait for credential saves after Back or Cancel.` (`## Plan follow-ups`)
- `Reveal the credential editor when Save and Close fails.` (`## Plan follow-ups`)
- `Stop Settings Save and Close when an independent save fails.` (`## Plan follow-ups`)
- `Bind Settings footer Save and Discard to the page and drafts shown.` (`## Plan follow-ups`)

## Acceptance criteria

1. **Credentials Back/Cancel during save:** In `settings/credentials/editor.rs`, Back and Cancel are disabled while a
   credential save/delete is in flight (and `leave()` refuses while `busy()`), matching `settings/recipes/editor.rs`,
   eliminating detached-editor save tracking.
2. **Reveal hidden credential editor on Save and Close failure:** When Save and Close fails on a retained credential
   draft hidden behind "Resume editing credential" (`credentials.key_revealed`), reveal that exact credential editor
   with its replacement secret and error visible.
3. **Name each custom-agent draft on Close (triage's small scope):** When multiple custom-agent forms (or Add custom
   agent + a registered agent) have unsaved changes, name each affected draft in one `HostSentence` line and reveal a
   page only when one section is affected (staying on the current page when multiple sections are affected). Do not
   build a custom multi-row scrollable prompt.
4. **Keep credential delivery warnings visible on Close (triage's small scope):** When a Save and Close batch commits a
   credential replacement that returns host delivery warnings, switch to `Section::Credentials` so the delivery report
   is shown instead of calling `finish_close()`, then clear the warning bit so a later Close is unaffected. Do not build
   a multi-close deferred-reveal state machine.
5. **Stop Save and Close on independent save failure:** While a page Save batch is active, `ask_to_close` still watches
   independent concurrent editor saves (those not in `batch.active`); if the user chooses Save, transfer those
   independent saves' receipts to the close decision so an independent refusal stops queued close writes and reveals the
   error while letting the earlier page Save finish.
6. **Bind footer Save and Discard to shown page and drafts:** Settings footer Save and Discard act only on the presented
   section and drafts (`ActionFrame` + `ApprovedEdit::permits` + section-keyed footer button IDs), so navigation or a
   late refusal cannot redirect Save or Discard onto another page's draft.
7. Targeted GUI tests cover every item above (plus light/dark visual verification where UI output changes).

## Decisions already made

- **Merge all 6 items into one ordered plan:** Avoids 6-way conflicts across `settings/editing.rs`.
- **Credentials Back/Cancel while saving:** Disable Back and Cancel while saving (matching Recipes) instead of keeping
  them clickable and tracking detached saves.
- **Custom-agent draft names & delivery warnings:** Keep triage's small scope (one sentence naming each custom-agent
  draft + reveal only for one section; simple keep-open-on-Credentials bit for fresh delivery warnings).
- **Existing SPEC rules kept:** Keep Settings interactive during Save and Close and keep the two Tensorlake key drafts
  as specified in `SPEC.md` (while honoring the maintainer's general low-latency-helm principle for Settings recorded by
  `settings-hook-save-notice`: do not add unnecessary async workarounds).
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter below.

## Implementation outline and requirement sources

- **User request & decisions:** The six `TODO.md` entries listed above, narrowed by the maintainer's settled decisions
  above (which explicitly override executor-proposed expansions in the `TODO.md` notes for
  `Name each custom-agent draft`, `Keep credential delivery reports visible`, and
  `Wait for credential saves after Back or Cancel`).
- **Binding repository constraints:** `SPEC.md` ("Settings", "Credentials", "One GUI at a time") and `SPEC_impl.md`
  ("Questions: the prompt").
- **Planner-proposed implementation choices (scope-reviewed, 4 ordered PRs):**
  - **PR 1 (Credentials Back/Cancel + hidden editor reveal):** In `settings/credentials/editor.rs`, disable Back/Cancel
    while `busy()` and guard `leave()` against `busy()` (matching `recipes/editor/leaving.rs:87`). In
    `settings/editing.rs::reveal_edit_failure`, clear `credentials.key_revealed` when `Edit::Credential(ed)` matches
    `credentials.editor()`. Add focused tests and golden check for the revealed editor.
  - **PR 2 (Custom-agent close names + credential delivery warnings):** In `settings/editing.rs`, check distinct
    `Section`s for page reveal and name each custom-agent draft (Add / registration display name) in one `HostSentence`.
    In `edit_finished`, inspect a 1-line `Credentials::has_undelivered()` getter on `Edit::Credential(_)` (since
    `SaveReceipt::finish` runs via `cx.defer` after `close_editor` populates `undelivered`) to set a
    `has_delivery_warnings: bool` flag on the close batch — showing `Section::Credentials` instead of calling
    `finish_close()` and clearing it, without widening `SaveReceipt`.
  - **PR 3 (Independent save refusal during page batch):** Keep `SaveReceipt` single-owner (`Option<OnSaveSettled>`).
    `batch.active` already partitions batch-owned vs independent edits: in `ask_to_close`, call `watch_close_question`
    for every `pending` edit not in `batch.active`, and on Save transfer only those independent edits onto
    `batch.after = AfterBatch::SaveAndClose` so a refusal stops queued writes and shows the error while letting
    `batch.active` finish.
  - **PR 4 (Footer Save/Discard binding):** Key footer Save/Discard button IDs by `self.shown.key()` (preserving focus
    across multi-field saves on the same page while preventing GPUI from carrying a mouse-down across page switches),
    and reuse `prompt::ActionFrame` + `ApprovedEdit::permits` in `editing.rs::page_answers` / `save_page` /
    `discard_page` (matching all 10 existing `ActionFrame` sites in `farhelm-gui` and `SPEC.md`'s 2026-11-21 decision,
    without adding a custom `Button::on_primary_press` down/release state machine).

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

Right after the checkout check, resolve the plan's record with
`cargo xtask records path plan/settings-close-save-hardening` (`plans/.agents/outside-files.md`, "Files outside the
repository") and invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already exists, read it
in full and resume where the previous session left off — cross-checking against GitHub and jj state per
`plans/.agents/executing.md`, "Executing one plan" — rather than starting over. If it does not exist, this is a fresh
start.

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

- **Status file:** `farhelm-plan-settings-close-save-hardening-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files", never inside the `plan/settings-close-save-hardening`
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
- Name bookmarks `plan/settings-close-save-hardening/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never
  merge.
- Restructure existing PRs in the stack (including ones from an earlier run) instead of stacking corrections on top.
- Never edit anything under `plans/`.
- The last code PR removes all six covered entries from `TODO.md`.
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

Every PR in the stack exists on GitHub, has passed its Opus 5.5 high review gate, removes all six covered `TODO.md`
entries in the last code PR, and satisfies the latest `## Decisions` entry (if any). Delivering the report is the
executor's closing step, not part of this goal.

## Release note

No release note needed.

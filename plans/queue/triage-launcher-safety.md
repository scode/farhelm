# Session launcher keyboard and confirmation safety fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

**Dependency:** This plan runs after `launcher-consent-and-custom-agent-fixes.md` lands on `main`, because both modify
`crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs`, `launcher/view.rs`, `launcher/view/search/`, and
`launcher/view/checkout.rs`.

## The goal

Execute five settled `TRIAGE_OUTCOMES.md` entries covering session launcher accessibility, keyboard guardrails, stale
permanent YOLO confirmation feedback, and repository-history error display, per
`review_feedback_queue/.agents/execute-outcomes.md` with maintainer-approved grouped PRs (one linear 2-PR stack):

1. `trailing-replace-with-recent-named-fill-enter-replaces.md` (`fix code`, high)
2. `trailing-launcher-new-clone-enter-after-vanished-pick.md` (`fix code`)
3. `trailing-launcher-held-escape-discards-draft.md` (`fix code`)
4. `trailing-launcher-stale-permanent-yolo-answer-silent.md` (`fix code`)
5. `trailing-repository-history-failure-shown-as-no-matches.md` (`fix code`)

Each PR graduates its covered entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (with final
Execution record, change ID, bookmark, and PR URL) and removes their feedback files and `review_feedback_queue/INDEX.md`
lines in the same PR.

### Acceptance criteria

1. **PR 1 (`plan/triage-launcher-safety/01-replace-recent-a11y-and-vanished-enter`) —
   `trailing-replace-with-recent-named-fill-enter-replaces.md` and
   `trailing-launcher-new-clone-enter-after-vanished-pick.md`:**
   - Keep click=fill and Enter=replace on Recent setups in Replace with, and update the highlighted option's accessible
     name/description in `crates/gui/farhelm-gui/src/launcher/view/search/start.rs` so assistive technology explicitly
     announces that Enter replaces and deletes the source session (while clicking fills), with a regression test.
   - Extend the existing presented-frame `ActionFrame` Enter guards in `launcher/view/search.rs` and
     `launcher/view/folder_list.rs` (for vanished empty-pane highlights and folder dropdown highlights) from Replace
     with only to New and Clone as well, matching `SPEC.md` ("One GUI at a time" and launcher choice disappearance
     rules), with regression tests.
2. **PR 2 (`plan/triage-launcher-safety/02-held-escape-stale-yolo-and-repo-error`) —
   `trailing-launcher-held-escape-discards-draft.md`, `trailing-launcher-stale-permanent-yolo-answer-silent.md`, and
   `trailing-repository-history-failure-shown-as-no-matches.md`:**
   - Call `focus::note_closing(true, window, cx)` whenever `Launcher::on_dismiss` (`launcher/view.rs`) closes an inner
     region (`inline_results` or `save_panel`) and returns `Dismissal::Stay`, preserving fresh second Escape to close
     the launcher, with tests for both regions.
   - Check `inline.is_open(cx)` in `yolo_inline` (`launcher/view/yolo_question.rs`) so a closed inline slot never hides
     refusal alerts; give pre-write stale permanent YOLO answers visible refusal feedback consistent with the one-off
     answer (including when Save as template is open); show a clear launcher note on post-write staleness explaining
     that the host now starts YOLO sessions without asking and the launch did not start because it changed; update/add
     tests.
   - Preserve repository-history load errors in `checkout.repositories` (`launcher/view/checkout.rs`) and show
     `"Couldn't load recent repositories; enter owner/repo"` in `launcher/search.rs` (`repository_empty_message`) while
     keeping manual `owner/repo` entry and the once-per-opening read (`SPEC_impl.md`), with a regression test.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped plan and grouped PRs:** Approved by the maintainer on 2026-11-25.
- **Replace with Recent setups:** Keep click=fill and Enter=replace; fix screen-reader accessible naming/description.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation"),
  `review_feedback_queue/.agents/execute-outcomes.md` (with the maintainer-approved grouped PRs above),
  `.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`.
- Never mutate process environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live
  Farhelm install.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`launcher/view/search/start.rs`, `launcher/view/search.rs`, `launcher/view/folder_list.rs`):**
   - Reuse existing `ActionFrame` (`start_enter_frame` / `enter_can_replace`) by removing the `Mode::ReplaceWith`-only
     gate rather than inventing a second frame guard.
2. **PR 2 (`launcher/view.rs`, `launcher/view/yolo_question.rs`, `launcher/view/checkout.rs`, `launcher/search.rs`):**
   - Reuse `focus::note_closing(true, window, cx)` directly in `on_dismiss`; represent `checkout.repositories` as a
     small tri-state (`Loading` / `Loaded(Vec<_>)` / `Failed`) rather than `Option<Vec<_>>` with `unwrap_or_default()`.

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
`farhelm-plan-triage-launcher-safety-log.md` in the parent directory of the executing checkout. If the log already
exists, read it and resume where the previous session left off (cross-checking against VCS state and open PRs) rather
than starting over. If it does not exist, this is a fresh start.

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
`plan/triage-launcher-safety/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge. Never edit
files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on top. Graduate each
covered outcome to `TRIAGE_OUTCOMES_ARCHIVE.md` and remove its feedback file and `review_feedback_queue/INDEX.md` entry
in the same PR.

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
`farhelm-plan-triage-launcher-safety-watchdog.json` beside the working log, watching the executing checkout root,
`$HOME`, `/tmp`, the log's parent directory, your scratch directory, and any worktrees on other filesystems, at a
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

Every PR in the stack exists as a draft on GitHub, satisfies its acceptance criteria and the latest `## Decisions` entry
(if any), graduates its covered `TRIAGE_OUTCOMES.md` entries and removes their queue files/index lines, and has passed
its `gpt-6.2-sol` high review gate. Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

# Host confirmations, Templates footer, row menu, and terminal notice fixes

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says,
except where this goal explicitly amends them.

## The goal

Execute eight settled desktop GUI `TRIAGE_OUTCOMES.md` entries outside Settings and the launcher, per
`review_feedback_queue/.agents/execute-outcomes.md` with maintainer-approved grouped PRs (one linear 3-PR stack):

1. `trailing-host-refusal-remove-reads-live-skip.md` (`fix code`, clearly settled; ~100-line size gate)
2. `trailing-host-setup-frozen-skip-ignores-later-restore.md` (`fix code`; maintainer simplicity kick-back below)
3. `trailing-tensorlake-rerun-reads-skip-setting-late.md` (`fix code`; maintainer simplicity kick-back below)
4. `trailing-templates-stale-save-duplicate-footer.md` (`fix code`, clearly settled; ~100-line size gate)
5. `trailing-menu-stop-and-suspend-exited-shell-wording.md` (`fix code`)
6. `trailing-file-disclosure-holdback-silent-click.md` (`fix code`)
7. `trailing-file-prepaint-click-test-gap.md` (`other`, Nov 21 rule)
8. `trailing-reposted-notice-not-revealed.md` (`fix code`)

Each PR graduates its covered entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (with final
Execution record, change ID, bookmark, and PR URL) and removes their feedback files and `review_feedback_queue/INDEX.md`
lines in the same PR.

### Acceptance criteria

1. **PR 1 (`plan/triage-gui-hosts-menus-and-terminal/01-host-confirmations-and-skip-prefs`) —
   `trailing-host-refusal-remove-reads-live-skip.md`, `trailing-host-setup-frozen-skip-ignores-later-restore.md`, and
   `trailing-tensorlake-rerun-reads-skip-setting-late.md`:**
   - In `crates/gui/farhelm-gui/src/hosts/refusal.rs`, snapshot the drawn host view and `remove::asks_first(cx)` via
     `ActionFrame` and call `remove::start_shown` (matching `hosts/menu.rs`), with a GPUI regression test (respecting
     its ~100-line clearly-settled size gate).
   - In `crates/gui/farhelm-gui/src/hosts/add.rs` and `hosts/setup.rs`, check
     `asks_before_setup || setup::asks_first(cx)` when the SSH plan settles so turning skip OFF mid-probe restores the
     confirmation while turning skip ON mid-probe still cannot skip it, with a regression test.
   - In `crates/gui/farhelm-gui/src/hosts/setup.rs` (`SetupRerun::Tensorlake` arm of `setup::rerun`), keep the fix local
     to `hosts/setup.rs`: fire the existing `GET /api/tensorlake` (`operations::read::<Settings>`) read at press time
     (before/alongside probing, defaulting to ask on failure/unavailability) and require both the pre-probe and
     post-probe reads to report `create_without_asking == true` before skipping confirmation, with a regression test. Do
     **not** add a new pushed `Collection` or menu-open fetch.
   - **Maintainer simplicity kick-back rule (applies independently to items 2 and 3):** Only implement each of those two
     skip-preference timing fixes if it stays simple. If either turns complicated (for example, wiring Tensorlake's
     pre-probe read cleanly with `Mark` / `launch_in_flight` / `setup_sign_in::plan`), stop work on that item, record a
     concrete entry in `TODO.md` under `## Plan follow-ups` explaining why it turned complicated, graduate its
     `TRIAGE_OUTCOMES.md` entry to `TRIAGE_OUTCOMES_ARCHIVE.md` as deferred to that `TODO.md` entry (removing its queue
     file and index line), log a `DECISION`, and continue with the remaining items.
2. **PR 2 (`plan/triage-gui-hosts-menus-and-terminal/02-templates-footer-and-menu-stop-suspend`) —
   `trailing-templates-stale-save-duplicate-footer.md` and `trailing-menu-stop-and-suspend-exited-shell-wording.md`:**
   - In `crates/gui/farhelm-gui/src/templates/editor.rs` and `dialog.rs`, bind the Templates editor footer's Save and
     Duplicate buttons to the drawn editor instance via `name_focus` / `dialog.is_open_editor(&name_focus, cx)`
     (matching Delete), ignoring stale clicks after switching templates, with a regression test (respecting its
     ~100-line clearly-settled size gate).
   - In `crates/gui/farhelm-gui/src/session_list/menu.rs`, `environment.rs`, and `environment_ui.rs`, give the row
     menu's Stop and suspend item (tooltip, description, confirmation, and Cancel copy) a tab-only variant when the
     session's main program has already ended while terminal tabs remain open (matching PR #4931's wording in
     `stop.rs`), with tests and updated goldens if affected per `.agents/validation.md`.
3. **PR 3 (`plan/triage-gui-hosts-menus-and-terminal/03-terminal-file-holdback-and-notices`) —
   `trailing-file-disclosure-holdback-silent-click.md`, `trailing-file-prepaint-click-test-gap.md`, and
   `trailing-reposted-notice-not-revealed.md`:**
   - In `crates/gui/farhelm-gui/src/terminal/attached.rs`, emit the existing `"not visible"` refusal
     (`Hidden::NotVisible.file_words()`) when clicking a terminal file path whose disclosure was held back over the
     pointer (matching web links and `SPEC_impl.md:3555-3562`), with a regression test.
   - Apply the Nov 21 rule to `trailing-file-prepaint-click-test-gap.md`: tighten the test in `terminal/files/tests.rs`
     if about 20 lines or fewer; otherwise leave untouched and record which way it went.
   - In `crates/gui/farhelm-gui/src/notices.rs` and `main_view.rs`, move a re-posted notice to the bottom of the notice
     stack in `notices::post` and scroll it into view in `main_view.rs`, with a regression test.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped plan and grouped PRs:** Approved by the maintainer on 2026-11-25.
- **Skip-preference timing items (`trailing-host-setup-frozen-skip-ignores-later-restore.md`,
  `trailing-tensorlake-rerun-reads-skip-setting-late.md`):** _"only if it stays simple. if this turns complicated it is
  not worth it - kick back into todo list."_
- **Nov 21 rule on `trailing-file-prepaint-click-test-gap.md`:** Fix only if about 20 lines or fewer and guarding
  user-facing behavior; otherwise drop with no change and record which way it went.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation"),
  `review_feedback_queue/.agents/execute-outcomes.md` (with the maintainer-approved grouped PRs above),
  `.agents/validation.md` (including GUI goldens if affected), `.agents/test-evidence.md`, and
  `.agents/test-authoring.md`.
- Never mutate process environment variables in tests; keep `scripts/check-test-sleeps.py` clean; never touch the live
  Farhelm install; any deferred `TODO.md` entry goes in `## Plan follow-ups`.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

1. **PR 1 (`hosts/refusal.rs`, `hosts/add.rs`, `hosts/setup.rs`):**
   - Reuse existing `ActionFrame` + `remove::start_shown` in `hosts/refusal.rs`, and `setup::asks_first(cx)` in
     `hosts/add.rs` and `hosts/setup.rs`.
   - For Tensorlake Re-run, Tensorlake's `create_without_asking` is not in any pushed `Collection`; keep the pre-probe
     read strictly local to `SetupRerun::Tensorlake` in `hosts/setup.rs` via `operations::read::<Settings>` (or invoke
     the maintainer's simplicity kick-back if wiring it cleanly gets complicated).
2. **PR 2 (`templates/editor.rs`, `templates/dialog.rs`, `session_list/menu.rs`, `session_list/environment.rs`,
   `environment_ui.rs`):**
   - Reuse existing `dialog.is_open_editor(&name_focus, cx)` directly; add a small tab-only enum/flag to
     `ProviderUi::suspend_question` / `suspend_action` / `suspend_cancel` rather than duplicating the dialog.
3. **PR 3 (`terminal/attached.rs`, `notices.rs`, `main_view.rs`):**
   - Reuse `Hidden::NotVisible.file_words()` directly; track a monotonic post sequence or position change in
     `main_view.rs` so re-posted banners trigger `reveal_latest`.

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
`farhelm-plan-triage-gui-hosts-menus-and-terminal-log.md` in the parent directory of the executing checkout. If the log
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
`plan/triage-gui-hosts-menus-and-terminal/<nn>-<short-name>`. Keep PRs as drafts — never mark one ready and never merge.
Never edit files under `plans/`. Restructure existing open PRs of this plan instead of stacking corrections on top.
Graduate each covered outcome to `TRIAGE_OUTCOMES_ARCHIVE.md` and remove its feedback file and
`review_feedback_queue/INDEX.md` entry in the same PR.

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
`farhelm-plan-triage-gui-hosts-menus-and-terminal-watchdog.json` beside the working log, watching the executing checkout
root, `$HOME`, `/tmp`, the log's parent directory, your scratch directory, and any worktrees on other filesystems, at a
60-second interval with default thresholds. Follow the `scode-build-goal` skill's Resource watchdog section for harness
wiring, startup verification, acting on alerts, and stopping the monitors at close.

### Decision logging and unattended fallback

Log major design or tradeoff choices in the working log as `DECISION` with alternatives considered.

This plan runs unattended. Never ask the maintainer anything in conversation — no direct questions, no harness question
tool, no optional follow-up offers, and no turn ending waiting for confirmation. Decide routine implementation forks
within the agreed scope and log them as `DECISION`. If a genuine fork requires a maintainer decision (material scope
expansion, weakened guarantee, omitted requirement, clearly-settled size gate exceeded without a TODO kick-back rule, or
unresolved design conflict), push all consistent work and block per `plans/.agents/executing.md`, "Executing one plan"
(step 10, Blocking). Things the maintainer should know and possible follow-ups go into the delivery report.

### Done criterion

Every PR in the stack exists as a draft on GitHub, satisfies its acceptance criteria and the latest `## Decisions` entry
(if any), graduates its covered `TRIAGE_OUTCOMES.md` entries and removes their queue files/index lines, and has passed
its `gpt-6.2-sol` high review gate (with any kicked-back PR 1 skip-preference item documented in `TODO.md`
`## Plan follow-ups` and logged as `DECISION`). Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

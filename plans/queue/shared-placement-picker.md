# Shared placement picker: drop a no-op press, give it a home

Written against main at 04631d86 on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

This plan covers two TODO.md entries (bucket "Near term"), merged with the maintainer's agreement because the second is
exactly the test coverage the first needs. Verbatim:

> **Shared placement picker: drop a no-op event and give it a shared home.** Since gpui-shared-placement (#4068), the
> launcher and the template editor share one placement picker (`crates/gui/farhelm-gui/src/launcher/view/placement.rs`).
> Pressing an already chosen "New" segment that has nowhere to go sends a "New, no target" change that the launcher
> ignores and the template editor needs its own guard against (it would otherwise move the template to Existing host);
> that guard is untested. Drop the press inside the shared picker and let the change carry only the target (about 10
> lines less, and the guard and its comment go). Separately, the shared picker lives in a launcher-private file that the
> GUI ownership table (`docs/gpui/foundation-ownership.md`) does not cover, so a change there alters the template editor
> without the ownership check reporting it: move the shared items to a sibling module and add an ownership row. Found by
> a skeptical re-review of GPT-6.2 Sol plans; recorded by the maintainer on 2026-11-18.

> **Test that the template editor ignores New sandbox or New container with nothing left to place it on.** The launcher
> and the template editor share one placement picker (#4068). The editor ignores a press on the already chosen New
> sandbox or New container segment once nothing is left to place it on (the last container host removed while the editor
> was open, say), where passing the press on would move the template to Existing host; the launcher ignores the same
> press on its own. `docs/gpui/parity/templates-and-quick-launch.md` states the behavior, but no test covers it, so a
> later change to the shared picker could break it unnoticed.

### Acceptance criteria

1. The shared placement picker (`crates/gui/farhelm-gui/src/launcher/view/placement.rs`, `build_placement_rows`) drops a
   press on the already chosen "New" segment itself, so the change it reports carries only the target:
   `PlacementEdit::RunsOn` holds an `Option<LaunchTarget>` rather than a segment and a target. The launcher calls
   `draft.choose_runs_on(RunsOn::of(target.as_ref()), target)`, as `templates/apply.rs` already does, and keeps
   `LaunchDraft::choose_runs_on`'s signature.
2. The template editor's own guard against that press (`templates/editor/rows.rs`, around lines 516 to 522) and its
   comment are deleted, and so are the doc lines in `placement.rs` that describe the no-op change.
3. A template editor test covers the behavior the guard used to protect: with a container template open while no listed
   host can hold containers (or with the last such host removed while the editor is open), pressing the chosen "New
   container" segment leaves the template targeting New container and shows no Host select. The test asserts its premise
   (no listed host can hold containers) before the press. A starting point could be
   `templates::editor_tests::retained_choices_and_an_unusable_only_host_stay_in_reach`, but that fixture still has a
   usable place for New container, so its fleet needs changing. A launcher-side twin is optional.
4. `PlacementMode`, `PlacementEdit` and `build_placement_rows` move from the launcher-private
   `launcher/view/placement.rs` to a module of their own under `crates/gui/farhelm-gui/src/launcher/`, which
   `view/placement.rs` keeps for the launcher-only account read, Podman checks and notes. The re-export in
   `launcher/view.rs` goes, and the module docs are split between the two files so each describes what it holds.
5. The other piece the launcher shares with the template editor, the checkout destination choice (`destination_choice`
   in `launcher/view/checkout.rs`, the Folder / Managed checkout choice, used by `templates/editor/rows.rs`), moves into
   the same module. That choice is not placement, so name the module for what both share (rows the launcher and the
   template editor both draw), not `placement_picker`; the name is yours, logged.
6. `docs/gpui/foundation-ownership.md` gains one row (with its subsection and extension-path row) for that module, owned
   by the launcher area and shared with the template editor (quick launch never draws these rows). The ownership check
   script that once enforced that table was removed when the GPUI migration ended (the doc's line 3 says so), so nothing
   will report a missing row; the row is still the documented owner map, and the TODO entry and the maintainer ask for
   the move. Keep every cell no wider than its column's widest, in both tables, so other rows are not re-padded.
7. `docs/gpui/parity/templates-and-quick-launch.md`'s note on the "Runs on switch's two small differences" says what is
   true afterwards.
8. No user-visible change, so no changelog fragment is needed unless a PR's type calls for one. The last PR removes both
   TODO.md entries.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, where they asked for the recommended choices unless there
was a strong reason not to:

- **The template editor test entry is part of this plan.**
- **The checkout destination choice moves too** (criterion 5).
- **No release note** (see Release note).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the two TODO entries and the Decisions above.

**Binding repository constraints:** root `AGENTS.md`, "Code rules" (provider decisions stay in the provider UI module;
no comparisons of providers in shared code), "Conventional Commits", "Tests and validation";
`.agents/test-authoring.md`.

**Planner proposals** (from reading the code at 04631d86; follow the code where it differs, logged as a DECISION):

- Runs on is a button group (`Role::Group`), which reports every press, the chosen option included
  (`components/radio_group.rs`, `Choice::choose`). Its `on_change` sends `PlacementEdit::RunsOn(segment, target)` with
  `target` `None` for a chosen "New" segment with nowhere to go; that is the only case the target is missing.
- The moved items depend only on `request::{container_reason, is_container_host}`, `ProviderUi` and components, not on
  `Launcher`, so the move is mechanical.
- About 20 tests call `LaunchDraft::choose_runs_on`; keeping its signature keeps them as they are.

## Outline

1. `plan/shared-placement-picker/01-drop-noop-press`: criteria 1 to 3. `refactor:` (or `test:` if the test dominates),
   no fragment.
2. `plan/shared-placement-picker/02-shared-module`: criteria 4 to 7. `refactor:`, no fragment. Removes both TODO.md
   entries.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-gui`, the GUI crate's launcher and templates unit tests through `python3 scripts/record-test-run.py`, and
`dprint check` on the changed Markdown. No golden run: nothing drawn changes.

## Fallbacks

- If the press cannot be dropped inside the picker without the picker knowing about launcher-only state, keep the drop
  in the picker's own terms (the chosen segment and an empty target) and log it.
- If `destination_choice` turns out to depend on launcher-private state that does not move cleanly, leave it where it
  is, add its ownership row as shared, and log why.

Decisions to log: the module's name and place, what moved, every test changed and why, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-shared-placement-picker-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-shared-placement-picker-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests on the machine you run on, as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`. The maintainer chose this on 2026-11-18; do not use the boat sandboxes
(`.agents/boat.md`) for this plan.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (one of its remotes
points at that GitHub repository; if not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill
with the log file's absolute path. If the log exists, read it and resume where the previous session left off,
cross-checking it against reality (the jj graph, bookmarks and open PRs) rather than starting over. If it does not
exist, this is a fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the
resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/shared-placement-picker/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entries.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

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

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last code PR removing this plan's
TODO.md entries. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.

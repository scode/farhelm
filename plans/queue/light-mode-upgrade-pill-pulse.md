# Make the pending-update pill reminder visible in light mode and bold its arrow

Written against main at fea806bc on 2026-11-23. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Make the pending-update pill reminder visible in light mode and bold its arrow.** In light mode, the ready-update
> pill's resting, hover, and pulse backgrounds (`#a63e4d` at 14.1%, 15.3%, and 16.5% alpha over `#e9edf4`) differ by
> only 1–3 RGB levels because `#a13443` text is already near 4.5:1 contrast at rest, so the reminder pulse is nearly
> invisible (especially when the arrow is hidden). Keep the tinted pill at rest (`#e0d4dc` / `#a13443`, 4.74:1), step
> hover to a 22% wash with darker ink (`#8b2837`, 5.31:1), and flip the 2.5-second light-mode pulse to solid `danger_bg`
> (`#a83e4d`) with white `on_fill` text (6.07:1; switch the `⚠` warning glyph to white during the pulse so it stays
> readable). Also draw the bouncing `→` reminder arrow bold at 13 px instead of regular 12 px so it matches the pill's
> weight.

In product terms, sharpened by the maintainer's planning decisions: make the pending-update pill's hover and 2.5-second
reminder pulse clearly visible in light mode (resting tinted wash `#e0d4dc` / `#a13443`, hover 22% wash / `#8b2837`,
pulse solid `danger_bg` `#a83e4d` / white `on_fill`, with the `⚠` advisory warning glyph switching to white during the
pulse), and draw the bouncing `→` reminder arrow bold at 13 px in **both light and dark modes**. Refresh and publish the
affected GUI golden screenshots.

### Acceptance criteria

1. **Light-mode update-ready pill states:** In `Palette::LIGHT`:
   - Resting stays tinted `danger_wash` (`0xa63e4d24` over `#e9edf4` → `#e0d4dc`) with `danger_strong` (`#a13443`,
     4.74:1 contrast).
   - Hover steps to a 22% wash (`0xa63e4d38` → `#dac7cf`) with darker ink `#8b2837` (5.31:1 contrast).
   - The 2.5-second reminder pulse flips to solid `danger_bg` (`#a83e4d`) with `on_fill` (`#ffffff`, 6.07:1 contrast),
     and the `⚠` warning glyph in `update_readout/view.rs` switches to `on_fill` during the pulse so it remains
     readable.
   - Dark mode keeps its existing update-ready pill colors.
2. **Bold 13 px reminder arrow in both themes:** `crates/gui/farhelm-gui/src/update_readout/view/reminder.rs` draws the
   bouncing `→` arrow with `FontWeight::BOLD` at `px(13.)` in both light and dark modes (still fitting cleanly within
   `ARROW_ROOM = px(24.)`).
3. **Tests, goldens, changelog fragment, and TODO cleanup:** Add/update unit tests covering contrast ratios and
   `toggle_look(palette, hovered, pulsing)` foreground/warning colors in light and dark modes; pass GUI lints and crate
   layering checks; refresh and publish affected golden screenshots (`version-readout-update-ready--light.png`,
   `--dark.png`, and any other affected scene) via `scripts/check-gui-goldens.py` + `scripts/gui-goldens.py publish`;
   amend `releasing/changelog.d/pending-update-pill.md` if still present; update `SPEC.md`/`SPEC_impl.md` if they name
   the pill colors or arrow size; remove the covered entry from `TODO.md`.

## Decisions already made

### Maintainer decisions (binding)

- **Light-mode pill palette & warning glyph:** Keep resting tinted wash, hover 22% wash with `#8b2837`, pulse solid
  `danger_bg` (`#a83e4d`) with white `on_fill` (switching `⚠` to white during the pulse).
- **Arrow styling:** Bold 13 px `→` reminder arrow in **both light and dark modes**.
- **Review gate:** Single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.`

### Binding repository constraints

- `AGENTS.md`, "Changelog fragments": amending unreleased work edits its existing fragment
  (`releasing/changelog.d/pending-update-pill.md`) rather than adding a second one.
- `.agents/docs-media.md` / `.agents/validation.md`: golden screenshot changes are published through
  `scripts/gui-goldens.py publish` and verified with a clean `python3 scripts/check-gui-goldens.py` pass; never commit
  captured media on main.

### Planner-chosen implementation shape

- Do **not** generalize `Button::colors()` across all 18 `Tier` variants to add a `hover_ink` field.
  `VersionReadout::render()` already computes `is_hovered` and `VersionReadout::toggle()` already sets foreground ink on
  `version_text(...)` and `marks(...)`. Instead:
  - Update `LIGHT`'s `danger_wash_hover` and `danger_wash_pulse` in `crates/gui/farhelm-gui/src/theme.rs`, and add
    dedicated hover/pulse ink tokens on `Palette` (`danger_strong` in `DARK`; `#8b2837` and `on_fill` in `LIGHT`).
  - Have `Tier::UpdateReady { pulsing }` in `crates/gui/farhelm-gui/src/components/button.rs` return the pulsing ink
    token when `pulsing` so `Button::colors()` stays consistent.
  - Pass `is_hovered` into `VersionReadout::toggle()` and add a pure `toggle_look(palette, hovered, pulsing)` helper
    beside `check_look` in `crates/gui/farhelm-gui/src/update_readout/view.rs` returning `(ink, warning_ink)` so the
    resting/hover/pulse color contract (including `WARNING_MARK`) is directly unit-testable.

## Outline

### Existing mechanisms reused

- `crates/gui/farhelm-gui/src/theme.rs` (`Palette::LIGHT`, `Palette::DARK`, theme contrast tests).
- `crates/gui/farhelm-gui/src/components/button.rs` (`Tier::UpdateReady`).
- `crates/gui/farhelm-gui/src/update_readout/view.rs` (`check_look`, `toggle`, `render`'s `is_hovered`).
- `crates/gui/farhelm-gui/src/update_readout/view/reminder.rs` (`reminder_gap`).
- `scripts/check-gui-goldens.py` and `scripts/gui-goldens.py publish`.

### PR stack

1. **`plan/light-mode-upgrade-pill-pulse/01-light-pulse-and-bold-arrow`** — Light-mode pulse/hover palette, white
   warning glyph on pulse, bold 13 px arrow in both themes, tests, and golden refresh:
   - Update `theme.rs`, `components/button.rs`, `update_readout/view.rs`, and `update_readout/view/reminder.rs`.
   - Add/update theme contrast and `toggle_look` unit tests.
   - Regenerate, visually inspect, and publish affected golden screenshots via `scripts/check-gui-goldens.py` and
     `scripts/gui-goldens.py publish`, then confirm a clean compare pass.
   - Amend `releasing/changelog.d/pending-update-pill.md` if still present, update `SPEC.md`/`SPEC_impl.md` if needed,
     and remove the covered entry from `TODO.md`.

## Fallbacks

- If `releasing/changelog.d/pending-update-pill.md` was already consumed by a release before this plan runs, add a new
  `kind: changed` or `kind: fixed` fragment per `AGENTS.md`, "Changelog fragments".
- Scope-reassessment triggers (examples, not a blacklist): expanding `Button::colors()`'s return tuple across all 18
  tiers, changing dark-mode pill colors, or changing reminder timing/trigger behavior.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-light-mode-upgrade-pill-pulse-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-light-mode-upgrade-pill-pulse-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- PR 1: `farhelm-gui` tests (`theme`, `update_readout`, `button`), `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `python3 -B scripts/check-gui-lints.py`,
  `python3 -B scripts/check-crate-layering.py`, `python3 scripts/check-gui-goldens.py` (iterating on
  `--scene version-readout-update-ready`, then full `--update`, visual inspection, `scripts/gui-goldens.py publish`, and
  a clean compare pass), `python3 releasing/check-changelog.py format`, `dprint check` on changed Markdown, and
  `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`.

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

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
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Make the pending-update pill reminder visible in
  light mode and bold its arrow."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

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
alternatives considered: in particular those named under Planner-chosen implementation shape and Fallbacks above, and
every review finding you decided not to follow. The maintainer will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

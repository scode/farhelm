# Let every retained terminal line show after an agent exits

Written against main at 3617ee450 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **Expose the oldest terminal output after an agent exits.** The exit notice can lift the oldest retained line or the
> missing-output notice out of the pane even at the top of scrollback. Keep the exit notice below the text and make
> every retained row reachable, including when the agent exits on the alternate screen. Cover drag selection and Select
> All against the same visible history. Plan: `plans/queue/exited-terminal-scrollback.md`.

History: the 2026-11-21 triage approved fixing this (`exit-notice-hides-oldest-scrollback-row.md`, now in
`TRIAGE_OUTCOMES_ARCHIVE.md`). The plan `terminal-exit-and-placeholder-fixes` (#4414) built a working fix, found it over
that triage's size gate, and filed this TODO entry instead; #4414 changed only bookkeeping. Its report
(`plans/reports/terminal-exit-and-placeholder-fixes.report.md`) and its working log, if they are still around, are
background only; nothing below depends on them.

### Where it stands (from planning research at main 3617ee450)

- **The lift.** `placement` and `lift` (`crates/gui/farhelm-gui/src/terminal/attached.rs`, about lines 1814 and 1988):
  with an exit line, the screen needs `rows + 1` rows of height, and any shortfall lifts the shown rows by whole rows
  (at most the whole grid). A fitted pane has less than one spare row, so an exit normally lifts by one; fractional
  scales or the red status line (`band`) can make it more. Held notices lift nothing.
- **Drawing.** `screen` (about 2114) draws the view at `mt(-lift)` inside an `overflow_hidden` area (about 2392-2398)
  and places the exit line at `rows * cell_height - lift` (debug selector `terminal-exit`). Nothing there looks at the
  scroll position.
- **The clamp.** `crates/gui/farhelm-grid-model/src/viewport.rs` (`first`, about 60-74; `top` and `scroll_to`, about
  103-121) clamps the view's top to the oldest retained line, or to the notice line one above it when history was
  trimmed. On the alternate screen it returns the screen's top, so there is nothing to scroll. So at the scroll limit,
  the row the lift clips can never be seen.
- **The notice line** "Some earlier output could not be recovered": `ViewRow::Notice` / `ShownRow::Notice`,
  `TRIMMED_TEXT` (`crates/gui/farhelm-gui/src/terminal.rs`, about 278); `line_at_row` (about 578) maps it to the oldest
  line for selection.
- **Wheel after exit.** `wheel` (`attached.rs`, about 1473) sends the wheel to the program whenever
  `wheel_goes_to_program(mouse, alternate)`, never looking at `state.exit`, so an agent that exited on the alternate
  screen or with mouse reporting on keeps receiving the wheel and the view never scrolls locally.
- **Press after exit.** `press` (about 3957-3961) treats any mouse mode other than Off as tracking, regardless of exit.
- **Drag selection.** `selection_position` (about 3908) adds the lift and clamps rows above the grid to row 0, which at
  the scroll limit is the clipped row.
- **Select All.** `TerminalView::select_all` (`terminal.rs`, about 713) selects from `oldest_line()` to the screen's
  last line on the primary screen, but from `screen_base` on the alternate screen, leaving out the history the
  supervisor still sends after an alternate-screen exit.
- **The supervisor keeps that history**: `primary_grid` (`crates/farhelm-supervisor/src/grid_output.rs`, about 482-490)
  reads history from the primary grid whichever screen shows. The client hides it.
- `TerminalState::alt_screen` doc (`crates/farhelm-proto/src/grid.rs`, about 1126-1131): "The client shows no history
  while it does: scrolling a full-screen program's view is the program's business".
- SPEC.md, "Terminal experience": "A stopped or exited session's terminal stays viewable while its host is up … Viewable
  means what the terminal itself holds"; "The scrollback stays while a full-screen program shows and is there when the
  program exits"; the gap paragraph's "Some earlier output could not be recovered". Neither spec mentions the exit line.

### Acceptance criteria

1. After an exit, at the scroll limit, the first fully visible row is the oldest retained line, or the "Some earlier
   output could not be recovered" line when history was trimmed, for any lift depth. The exit line keeps its place, one
   row directly under the last row, and never covers a row. The following (live, not scrolled back) view looks exactly
   as today; no golden should change, and a changed golden is a red flag.
2. The same holds whatever lifts the picture, the red status line included, with or without an exit (Decisions).
3. After an exit on the alternate screen, the final frame stays at the bottom and the primary history (and the notice
   line when trimmed) is reachable above it by the wheel, Shift+PageUp/PageDown and the scrollbar. A **live** alternate
   screen keeps today's behavior: no history, and the wheel goes to the program.
4. Once the program has exited, it is treated as not using the mouse: the wheel scrolls locally, a plain press and drag
   select, a link click opens, and bare pointer moves send no reports, as if mouse reporting were off, whatever mode or
   screen it last reported (Decisions). All three reads of the mode (`wheel`, about 1491; `mouse_report`, about 3719,
   which `hover_moved` calls on every bare move and whose `send_user` follows the bottom and clears the selection;
   `press`, about 3960) go through one helper giving the effective mode (`Off`, and not alternate for the wheel, once
   `exit` is set). Without the `mouse_report` part, motion mode (1003) snaps a scrolled-back view to the bottom and
   clears a fresh selection on the next pointer move. The scrollbar becoming visible on hover for an exited terminal
   with no history, or under the red status line, is expected (the padding adds positions), not a regression.
5. Select All selects exactly what navigation can reach: history from the oldest line (the notice standing for it)
   through the screen, including after an alternate-screen exit, and never padding.
6. A drag past the top edge clamps to the first visible row, never to a padding row or a clipped row.
7. Tests (see Outline) cover the scrolled-back exited case by reading what was painted, a lift of two rows, the exited
   alternate screen, Select All after an alternate-screen exit, the press after exit, and the red-status-line case; the
   existing `the_wheel_goes_to_the_program_on_the_alternate_screen` stays as the live-alternate guard.
8. SPEC.md "Terminal experience" gains a sentence after "is there when the program exits": the terminal shows how the
   program ended on a line below its last row, and every retained line, including history under a killed full-screen
   program's final frame and the missing-output line, can still be scrolled to and selected; and the selection paragraph
   gains a clause that once the program has exited, its mouse reporting no longer applies. The `alt_screen` proto
   comment says "while the program runs". The docs that claim "no history on the alternate screen" or report-on-lift
   behavior match: `viewport.rs`, `terminal.rs` ("Scrollback", `scroll_position`, `scroll_page`, both `select_all`
   docs), `attached.rs` (`lift`, `placement`, `wheel`, `wheel_goes_to_program` if the helper wraps it), and the
   docstrings of `the_wheel_goes_to_the_program_on_the_alternate_screen` and
   `the_pointer_names_the_cell_drawn_under_it_on_a_lifted_screen`.
9. No supervisor, helm or wire-format change. The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Any lift, not only the exit line:** the same padding covers the red status line's lift, even while the program runs
  ("Yes, fix it for any reason the picture moves up").
- **An exited program does not track the mouse:** after exit a plain drag selects even if the agent had mouse reporting
  on (one shared predicate for wheel and press).
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort. No visual review: nothing new is drawn.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:** SPEC.md "Terminal experience" as quoted; `.agents/test-authoring.md`; the GPUI
goldens check (`.agents/validation.md`); root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and
validation", and its "Instructions by path" for every path you touch.

**Planner proposals** (from the earlier executor's working prototype and its two scope checks):

- **Blank padding positions in `Viewport`.** Give the viewport one blank navigation position above the first content per
  lifted row (for example `set_top_padding`, producing `ViewRow::Blank`; `ShownRow::Blank` already exists in the
  renderer). At the scroll limit the lift then clips only blanks. The prototype made room by moving the position origin
  from `n + 1` to `n + GRID_MAX_DIMENSION + 1`, so both the notice and the oldest line can sit at wire line 0 with up to
  a full grid of blanks below the notice's position; any equivalent bound covering the maximum lift is fine. Each scroll
  step moves one row and the following position keeps its alignment, so goldens should not change. Wheel,
  Shift+PageUp/PageDown and the scrollbar all go through `first`/`top`, so they agree with no per-input change.
- **The padding comes from the same measured lift** that draws the exit line (`Placement`), pushed to the view in
  `Render` beside `set_cursor_phase`; the view refreshes its rows only when the count changes.
- **Selection clamp:** clamp the drag row to at least the padding; if the lift clips every row, return `None`.
- **One predicate for "live alternate screen"**: `alt_screen && exit.is_none()` (the prototype's
  `Viewport::hides_history`), used by the viewport and by Select All alike (the prototype's Select All missed it).
- **Keep that predicate separate from the input-routing helper** (acceptance 4): one decides history visibility in the
  grid model, the other input routing in the owner. Merging them (masking `alt_screen` after an exit) would flip
  `Viewport::synchronize`'s alternate-screen reset and clear selections at the moment of exit.
- `Viewport::synchronize` resets the anchor only on `alt_screen` changes; if a later state clears `exit` while the
  alternate screen still shows, a scrolled-back anchor stays clamped to the bottom. Harmless; decide deliberately
  whether to key the reset on history becoming hidden (not on every change of the predicate, which would clear a
  selection at exit), and log it.
- **Ruled out by the earlier executor's scope checks** (do not re-derive): clipping fewer rows, lifting only while
  following, nudging only the oldest row, moving or hiding the exit line while scrolled back, and refitting after exit.
  Each either jumps rows when following resumes, breaks short panes, or has no scroll position when there is no history.
- **The decisive test must read the paint cache**, not `find_shown`/`line_at_row`, or it passes on the bug.

## Outline

1. `plan/exited-terminal-scrollback/01-oldest-line-reachable`: `fix:` the viewport padding from the measured lift (any
   lift), the selection clamp, the docs in `viewport.rs`, `terminal.rs` and `attached.rs`, grid-model unit tests for
   `first`/`row_at`/`notice` with padding and a notice at line 0, and the GPUI test
   `an_exited_terminal_reaches_the_start_of_its_history` (lift established before the wheel; reads the painted row at
   the first unclipped index for the oldest line and for the trimmed notice; checks the exit line's bounds are unchanged
   and a drag above the grid). Add a two-row-lift case (the red status line plus exit) and a red-status-line case
   without exit, as further iterations of the same test loop. Changelog fragment (`kind: fixed`).
2. `plan/exited-terminal-scrollback/02-exited-alternate-history`: `fix:` the live-alternate predicate in the viewport
   (history and notice reachable once exited), the exited-program-does-not-track-the-mouse predicate for wheel and
   press, Select All on the live-alternate predicate, the `alt_screen` proto comment, the SPEC.md sentence, tests for
   the exited alternate screen, Select All after an alternate-screen exit including history, and a plain drag after exit
   with mouse reporting on, plus a motion-mode case (move the pointer after the release and after a wheel scroll; the
   selection and scroll position survive). Edits PR 1's fragment rather than adding a second. Removes the TODO entry.

## Fallbacks

- If the padding cannot be plumbed through the viewport as proposed, any shape that meets the acceptance criteria is
  fine (log it as a DECISION); the ruled-out alternatives above stay ruled out.
- A golden that changes in the following view means the following alignment moved: investigate before accepting.
- The earlier prototype was 137 production-and-test lines with one test; with the full test list and docs, well over 250
  lines across both PRs is expected and is not by itself a sign the design grew. Run the scope reassessment only if the
  production code grows well beyond the mechanisms named here.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-exited-terminal-scrollback-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-exited-terminal-scrollback-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/exited-terminal-scrollback/01-oldest-line-reachable`,
  `plan/exited-terminal-scrollback/02-exited-alternate-history`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Expose the oldest terminal output after an agent
  exits"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

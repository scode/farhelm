# Bulk Stop and Delete act on what the marked bar showed

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Keep bulk Stop and Delete bound to what the user saw.** In the desktop session list's marked bar, the click reads
> fresh confirmation preferences and shell guards. Capture the drawn targets, guards, confirmation preference, and the
> environment scope in the control as the single-row `Actionable` path does. Check that the connection, host identity
> and environment Delete scope still match at acceptance and at queued dispatch. Reword or remove targets when facts
> change without widening consent. This is the rule in SPEC_impl.md's “Questions: the prompt” and SPEC.md's bulk
> actions. Cover changed confirmation preferences, a retargeted host and an unreachable environment that reconnects
> while Delete is pending. Plan: `plans/queue/bulk-actions-bound-to-drawn.md`.

The maintainer chose only the narrow fix decided at triage (Decisions below): capture what the marked bar drew and act
on that snapshot, as the single-row menu does. Reading that choice, the planner leaves out the entry's checks at
acceptance and queued dispatch (connection, host identity, environment Delete lane) and its retargeted-host and
reconnecting-environment cases.

### Where it stands (from planning research at main b34ceee9f)

In product terms: when the user presses Stop or Delete in the marked bar, the desktop app does not act on what the bar
showed. At the press it re-reads each marked session's current state and the current **Skip session confirmations**
setting. A Stop or Delete carries a precondition taken from the row ("refuse if terminal tabs are open", "refuse if the
agent is running"), and the helm refuses a request whose session has become more alive than that. Recomputing at the
press means that in the moment between a frame being drawn and the click being handled, a Skip session confirmations
change makes a marked Stop or Delete that would have asked go ahead at once, and a session that just opened tabs is sent
a request that allows them. The triage put this in its highest bucket (killing live processes without the required
confirmation); the likelihood is low. Once the bulk question is open it is already safe: it freezes its targets, rewords
in place, and accepts no answer until the new words are on screen.

- The bar: `SessionList::marked_bar` in `crates/gui/farhelm-gui/src/session_list/marking.rs` (about lines 617-720). The
  Stop and Delete buttons' `on_click` (about 686-688) captures only the action and calls
  `list.bulk_lifecycle(action, …)`. The bar already reads `self.marking.shown()` and computes the "all being deleted"
  unavailability per frame.
- The press: `bulk_lifecycle` in `crates/gui/farhelm-gui/src/session_list/bulk.rs` (about 485-541). It returns if a bulk
  question is open, then for each key of `self.marking.shown()` reads the store's current row and builds a `Target` with
  `Target::of` (about 238-330, through `frozen_as`), and reads `menu::skips_questions` live (about line 512). It leaves
  out targets whose environment is being deleted (their not-started line said at once), then runs the batch at once when
  nothing must ask or Skip is on, or asks (`ask_bulk`). An environment Delete always asks.
- `Target` (about 184-215) already holds everything the request and the question need, frozen from one row: the guard
  (`stop::guard`, `delete::guard`), consequence words, whether its row would ask, and the environment Delete lane
  (`environment_delete::asked_in`). It derives `PartialEq`. The module docs' first rule (about 10-14) says targets are
  frozen "from the row the store holds at the press", which this plan changes.
- `stop::guard` (`session_list/stop.rs`, about 70-75) sets `refuse_if_tabs_open` when the row lists no tabs;
  `stop::asks` asks only when the row lists tabs.
- The single-row pattern to copy: `prompt::ActionFrame` and `ShownAction` (`crates/gui/farhelm-gui/src/prompt.rs`, about
  1150-1222). `capture` keeps one snapshot per drawing site, reused while the data and offer stay equal, and
  `ShownAction::shown()` yields it only once that frame has been presented (GPUI can draw a dirty frame just to dispatch
  input). The row menu captures `menu::Actionable` this way (`session_list/menu.rs`, `RowMenu::action_frame` about line
  834, the capture about 909-916), and so does the session header (`session_pane/header.rs`, about 427).
- Tests to model on: `a_header_click_cannot_stop_a_session_that_has_not_drawn` and
  `the_delete_shortcut_refuses_a_newly_drawn_unshown_session`
  (`crates/gui/farhelm-gui/src/session_pane/header_tests.rs`, about 187-245), using `test_support::show_next_frame` and
  `test_support::click_before_redraw` (`crates/gui/farhelm-gui/src/test_support.rs`, about 738-748). Bulk tests live in
  `session_list/bulk_tests.rs`; `skip_session_confirmations_runs_bulk_stop_and_delete_unasked` (about line 853) and its
  fixture with a `skip` argument (about 1392-1402) are the nearest.
- Where the bar's state lives: `marking::Kept` (`marking.rs`, about 409-430) for the window's life, beside the bar's
  focus handles; the list's per-UI-session state `Current` (`crates/gui/farhelm-gui/src/session_list.rs`, about 396-440)
  holds the marks and the bulk batches, and a reset drops it.
- The specs already state the rule this plan brings the bar into line with: SPEC_impl.md, "Questions: the prompt" (about
  984-988: "The same rule starts at the control that asks the question or sends the request directly. Its handlers carry
  the drawn session key, row, skip preference and the parts of the environment's state that determine the command … The
  question and request use the snapshot that admitted the press."), and SPEC.md, "Session list" (about 2394-2406:
  "Changed facts or confirmation preferences never dispatch a destructive action by themselves."). Neither needs a
  change.

### Acceptance criteria

1. Each of the bar's Stop and Delete captures, through `prompt::ActionFrame`, a snapshot of what that frame drew: the
   frozen `Target`s for the marked rows shown (guard, consequence words, whether each asks, environment Delete lane,
   retired sessions left out of Stop as today) and the Skip session confirmations preference read from the same store.
   The button's handler acts on `shown()` of that snapshot; a press whose frame was never shown does nothing. The
   snapshot also records which store it was drawn from (the store's entity id, as `menu::Actionable` does), and a press
   does nothing when the current store is another one (a reset happened since that frame).
2. `bulk_lifecycle` builds no targets from the store and reads no preference: whether to ask or run at once, and every
   request's precondition, come from the snapshot. It still does nothing while a bulk question is open, and live state
   may still only remove targets (a session whose environment started being deleted gets its not-started line, as
   today).
3. Nothing after the press changes: the question, rewording, confirmation, per-host pacing, holds, not-started lines and
   mark clearing work as they do now.
4. The snapshot is rebuilt plainly on each frame the bar draws, as the bar's offers already are. A cache keyed on what
   changed is added only if a measurement with hundreds of marked rows (the listing cap is 500) shows a real cost, and
   the measurement, or the reasoning for not measuring, is logged either way.
5. Tests, in `session_list/bulk_tests.rs`: (a) with Skip off and a marked session whose Stop asks, turning Skip on just
   before a press on the frame that showed it does not dispatch: the question opens and nothing is sent; (b) with Skip
   on and a marked session drawn with no tabs, a press on that frame after the session gained tabs sends Stop with the
   drawn guard (refuse if tabs are open). A press on a newly drawn, not yet shown frame sends nothing.
6. The doc comments that say targets come from the row held at the press (bulk.rs module docs, `bulk_lifecycle`,
   `marked_bar`) say they come from the frame that showed them.
7. The PR carries a changelog fragment and removes the TODO entry entirely; no new TODO entry is added for the dropped
   checks.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Only the decided narrow fix:** the fix decided at triage, capturing the drawn targets (each `Target` with its frozen
  guard, consequence words and environment Delete lane) and the Skip session confirmations preference at the marked bar,
  through the same `prompt::ActionFrame` mechanism the single-row menu uses, with the buttons acting on that snapshot.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, narrowed by the decisions above; the triage decision for the
original finding ("capture the drawn targets and preference in the button handler, as the row paths do"), recorded in
`TRIAGE_OUTCOMES_ARCHIVE.md` (search `bulk-action-uses-live-shell-guard`), and the landing monitor's note on it in
`plans/reports/triage-gui-sessions.report.md`, "Things you should know that the report does not say".

**Binding repository constraints:**

- SPEC_impl.md, "Questions: the prompt", and SPEC.md, "Session list" (the bulk question paragraphs and "Lifecycle
  operations" for Skip session confirmations), quoted above.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and its "Instructions by
  path" for every path you touch.

**Planner proposals:**

- The planner's reading of "only the decided narrow fix": the checks the deferred TODO text added beyond the triage
  decision (host identity and environment Delete lane at acceptance and at queued dispatch, a connection check there,
  rewording an open question on a lane change) are left out. The last PR removes the whole TODO entry and adds no new
  entry for them (a planner instruction); the report's follow-ups name the dropped checks so the maintainer can ask for
  them again.
- The same-store check at the press is part of "as the single-row menu does", not one of the left-out checks:
  `menu::Actionable` carries the store's entity id and `allows` refuses a different store, and the bulk snapshot does
  the same at the press. It matters because each button's handler holds its own clone of the snapshot, so a press GPUI
  dispatches to a frame drawn before a reset would otherwise act on the old UI session's snapshot. It checks nothing at
  acceptance of the question or at queued dispatch.
- One snapshot type holding the store's entity id, the targets and the skip flag (deriving `PartialEq`), one
  `ActionFrame` per button. The offer can be the button's unavailability, so a button that turns available is not given
  an already shown snapshot. Where the frames live (per UI session in `Current`, or `Kept` beside the focus handles) is
  your call; it does not decide what a press across a reset does, the store check does. Log the choice.
- Factor the target building out of `bulk_lifecycle` into a function the bar calls, so the press and the bar cannot
  freeze rows differently and nothing is duplicated.
- Cost: `ActionFrame::capture` compares the new data with the last on every frame, and building a `Target` formats
  strings and escapes host text. The list redraws on its own at most at the ticker's rate
  (`crates/gui/farhelm-gui/src/ticker.rs`, 10 a second, only while a row pulses in an active window), and the bar
  already walks the marked rows each frame for its offers, so a plain rebuild is the default. Only if a measurement
  shows a real cost, rebuild the snapshot when the shown marks, the store or the preference changed and reuse it
  otherwise. Do not capture the store's collections wholesale: they are a deep clone.
- The existing tests that press the bar's Stop or Delete are all in `bulk_tests.rs`; some may press on a frame that was
  never presented. Give them `test_support::show_next_frame` where needed rather than weakening the shown rule.
- Fragment, `kind: fixed`, along the lines of: "Stop and Delete for marked sessions now act on what the window showed
  when you clicked, so a confirmation setting or session that changed at that moment can no longer skip the question."

## Outline

1. `plan/bulk-actions-bound-to-drawn/01-bar-captures-drawn-targets`: the snapshot and its frames in the bar, the buttons
   acting on it, `bulk_lifecycle` reading neither the store's rows nor the preference, the doc comments, the tests, a
   changelog fragment, and the TODO entry's removal. `fix:` (for example
   `fix: act on the marked sessions and confirmation setting as shown`), with a fragment.

## Fallbacks

- If a check at acceptance or queued dispatch looks necessary while you work (host identity, the Delete lane, the
  connection), do not add it: it is outside the narrow fix the maintainer chose. Log it and say in the report why you
  thought of it.
- If a measured cost would need more than a small cache in the list to fix (a change to the store's API, say), keep the
  plain rebuild-every-frame version, log the measurement, and report the cost as a follow-up.
- If the bar turns out to draw something from the snapshot differently than today (it should not: the bar draws counts
  and labels, not targets), that is a drawing change; stop and reassess scope before going on.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-bulk-actions-bound-to-drawn-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-bulk-actions-bound-to-drawn-watchdog.json` beside it.
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
  `plan/bulk-actions-bound-to-drawn/01-bar-captures-drawn-targets`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Keep bulk Stop and Delete bound to what the user
  saw"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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

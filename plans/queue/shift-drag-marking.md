# Shift+drag marks sidebar rows

Written against main at d480779c on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `session-group-follow-ups.md` has landed (its `INDEX.md` line says so). That plan reworks the
session list's drag code (`session_list/pin_drag.rs`), gives every row a grip, states that there is never a second drag
gesture, and adds scrolling at the list's edges during a drag; this plan builds on its final shape.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Shift+drag across sidebar rows should mark them too.** Today only Shift+click marks a range (from the open session
> to the clicked row), and holding Shift while dragging the pointer over several sessions marks nothing. The maintainer
> has reached for Shift+drag several times expecting it to multi-select. Make a Shift+drag over the session list mark
> the rows it sweeps, consistent with Shift+click's range rule, and update SPEC's marking gestures to match.

### Acceptance criteria

1. Pressing the pointer with Shift held on a session row (its body, where a click opens the session) and moving onto
   other session rows marks the range from the row where the press began to the row under the pointer, in displayed
   order, replacing any earlier marks, consistent with Shift+click's range rule.
2. The range follows the pointer live: sweeping back unmarks the rows passed, so the marks always run from the start row
   to the row under the pointer, like selecting text.
3. Releasing the pointer ends the sweep and is never also a click on any row (no session opens, no marks are cleared),
   then or later: no later press and release can fire a click left over from the sweep's press. A Shift press that never
   left its start row is an ordinary Shift+click, as today. A sweep that left its start row and came back to it, then
   released there, marks only the start row.
4. Escape during the sweep, or a release the app never sees, cancels it and restores the marks from before the press. So
   does the start row leaving the listing mid-sweep (deleted from elsewhere, say).
5. A sweep starts only from a session row. A press on a group header still toggles its collapse; a press on a grip is
   still a move. Crossing a collapsed group marks nothing inside it, since marks count only for displayed rows. With the
   pointer above or below the list, the range clamps to the session row nearest the pointer at that edge of what is
   shown (edge scrolling, criterion 6, then moves it on); over a group header, it keeps the last session row the pointer
   was over. A start row scrolled out of view still anchors the range (the list is virtualized: compute the range over
   the list's entries, not over drawn rows).
6. Near the list's top or bottom edge, the list scrolls during a sweep the same way the group plan's edge scrolling
   scrolls during a move, and the range follows the rows passing under the pointer.
7. The open session keeps its look, the marked rows and the "N marked" bar look as they do today, and the order hold
   (rows do not reorder while the pointer is over the list) holds during a sweep. Cmd/Ctrl+drag does nothing new.
8. There is still one drag gesture in the code: the sweep reuses the drag owner the pin grip uses (its pointer capture,
   lost-release cancel, Escape and order-hold activity), generalized so it can carry either a move or a sweep, rather
   than a second capture mechanism beside it.
9. SPEC.md's marking gestures paragraph (around its line 2220 at planning time) adds Shift+drag; SPEC_impl.md's "one
   grip gesture" passage and the session list's module docs describe the generalized drag owner.
10. Tests: the range itself is the existing Shift+click rule with a different anchor, which `marking_tests.rs` already
    covers, so new unit tests cover only how a pointer position becomes a row (over a header, outside the list, a start
    row scrolled away). In-window tests: a Shift drag across rows marks them and opens nothing; no stray click fires on
    a later press and release after a sweep; a drag that never leaves its start row equals Shift+click, and one that
    leaves and comes back marks only the start row; Escape, a lost release and the start row leaving the listing restore
    the earlier marks; the order hold holds; a plain drag still does nothing; edge scrolling extends the range. The
    existing pin-drag tests pass unchanged.
11. A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes this plan's TODO.md
    entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, which showed a mockup of a sweep from one row down to
another:

- **The range starts at the row where the drag began.** Rejected: starting from the open session as Shift+click does,
  and adding the swept range to existing marks.
- **Sweeping back unmarks.** Rejected: paint mode, where every row touched stays marked.
- **Escape or a lost release restores the earlier marks.** Rejected: keeping the partial sweep.
- **Stated on the page and not objected to:** a sweep starts only from a session row; crossing a collapsed group marks
  nothing in it; outside the rows the range clamps; Cmd/Ctrl+drag is out of scope; edge scrolling comes from the session
  groups plan, which is why this plan waits for it.
- **Release note:** the maintainer's own wording (see the section at the end).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending, after `session-group-follow-ups.md`.**

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the Decisions above.

**Binding repository constraints:** SPEC.md's marking gestures, the rule that marks count only for displayed rows, and
the order hold; the "never a second drag gesture" rule that `session-group-follow-ups.md` records (check its final form
on main); root `AGENTS.md`, "Code rules", "Changelog fragments", "Conventional Commits", "Tests and validation";
`.agents/test-authoring.md`.

**Planner proposals** (from reading the code at planning time, before the session groups plan landed; re-read
`session_list/` on main before starting, follow the code where it differs, and log it as a DECISION):

- Shift+click is read on the row's open control in its `on_click` (`session_list/row.rs`), and `row_clicked`
  (`session_list/marking.rs`) builds the displayed list without group headers and anchors at the open session. GPUI
  fires `on_click` only when the button comes up over the hitbox it went down on, which is why Shift+drag does nothing
  today.
- The pin grip's drag (`session_list/pin_drag.rs`) owns window-wide capture of moves and releases (`watch`), treats a
  lost release as a cancel, catches Escape, records row bounds for the rows it can move, and counts as activity for the
  order hold. Generalize that owner in the narrowest form, a kind (move or sweep) inside the existing drag state, with
  no trait and no rename; sharing one owner also keeps a move and a sweep from running at once. Do not copy its "row no
  longer drawn cancels" rule into the sweep. (Extracting only the capture listeners is a close alternative; your call,
  logged.)
- For which row is under the pointer, the list already reports each item's bounds by index (`bounds_for_item`, used by
  `row_fully_in_view`), which gives the entry index directly; that may be simpler than extending the pin drag's bounds
  recorder to every row. If the session groups plan already records bounds for every row, reuse that instead.
- The phantom click: the pin grip stops its press before the row's open control sees it, but a Shift press on the open
  control does register with it, and the sweep's capture then keeps the control from clearing that press on release. A
  later press elsewhere released over the start row would then fire a click. Clear or swallow that pending press when
  the sweep ends; a test covers it (criterion 10).
- `Marks::mark` with the start row as anchor, over the displayed list `row_clicked` already builds from the entries,
  applies the range; no new marking function is needed.

## Outline

1. `plan/shift-drag-marking/01-generalize-drag-owner`, `refactor:`: the pin drag's capture, cancel, Escape and recorder
   become one drag owner that can carry a move or a sweep, with no behavior change, proven by the unchanged pin-drag
   tests. Fold it into PR 2 if it is trivial.
2. `plan/shift-drag-marking/02-shift-drag-marks`, `feat:`: the sweep, edge scrolling, tests, SPEC.md and SPEC_impl.md
   edits, the fragment, and the TODO.md entry removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-gui`, the session list's marking, pin-drag and groups tests through `python3 scripts/record-test-run.py`, the
GPUI end-to-end smoke if it has a drag step that the refactor could break, and
`python3 releasing/check-changelog.py format`. No golden should change: the marked look is unchanged.

## Fallbacks

- If the session groups plan landed a drag owner that already carries more than one kind of drag, extend it; PR 1 may
  then be unnecessary.
- If the group plan's edge scrolling cannot serve a sweep without restructuring it, keep criterion 6 but build the
  smallest adapter; a second scrolling mechanism is a departure: run the scope reassessment.
- If `session-group-follow-ups.md` was abandoned rather than landed, so main has no edge scrolling to reuse, criterion 6
  drops out: build everything else, and name edge scrolling as a possible follow-up in the report.

Decisions to log: the drag owner's shape, how the range is computed over entries, how edge scrolling is reused, every
test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-shift-drag-marking-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-shift-drag-marking-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation".

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/shift-drag-marking/<nn>-<short-name>`, as
  the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's entry for the release, not an instruction to
  you: each PR still leaves the fragment its type calls for, describing what that PR built, and where what got built
  departs from the release note (a renamed control, a part left out, a limitation it does not mention), the fragment's
  context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entry.

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
TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

You can now shift-drag across sessions in the session list to select multiple sessions.

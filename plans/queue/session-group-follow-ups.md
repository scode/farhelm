# Session groups: mark the open session, indent members, drag into groups

Written against main at 04631d86 on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan's PRs write the new behavior into them.

## The goal

This plan covers two TODO.md entries (bucket "Near term"), merged into one plan with the maintainer's agreement because
both change how group headers and member rows are drawn and the drag work generalizes the same code. Verbatim:

> **Show the open session in a collapsed group, and indent group members.** Since session-groups (#4110 to #4114), when
> the open session belongs to a collapsed group, nothing in the sidebar shows which session is open until the group is
> expanded; the header's status dot shows only the most urgent member. Make the selection visible on the collapsed group
> (details to be decided; marking the header that holds the open session is one option). Separately, group members are
> not indented, so in a long group only the header and the heavier separator after the last group mark where it ends;
> indent members under their header. Recorded by the maintainer on 2026-11-18 from the plan's review.

> **Drag sessions into groups.** Drag and drop any session, pinned or not, into a session group, out of one, or between
> groups in the session list. The session groups plan adds groups with a row-menu "Move to group" action and lets only
> pinned sessions cross between pin areas by dragging; this extends dragging to every row. Recorded by the maintainer on
> 2026-11-17 while planning session groups.

Three things change in the desktop app's session list:

1. Group members are indented under their group's header, with a thin guide line.
2. A collapsed group whose members include the open session says so, with a blue bar on its header's left edge.
3. Any session, pinned or not, can be dragged by its grip into a group, out of one, or between groups. Dragging a marked
   session moves every marked session.

### Acceptance criteria

1. **Indent.** In an expanded group, every member row's content (status mark, title, second line, age, row controls) is
   indented 16 points (the maintainer's number). The open-session fill, the marked fill and its marker, the row's
   hairline and the drag grip stay full width at the row's left edge. A thin vertical guide line in the palette's
   `border_nested` color runs in the indent, clear of the status marks (around 20 points from the row's left edge;
   measure the real header and rows and place it where it reads as belonging to the header, logging the position), from
   the first member down to the last member's bottom edge, so the group's end is visible. The list is virtualized, so
   each member row draws its own segment of the line. Global pins and ungrouped rows are not indented; compact rows use
   the same indent. The separators keep their SPEC.md meaning; the lighter separator after a group's leading pins sits
   within the group's indent. A 16-point indent does not put member status marks exactly under the group name (the
   header's toggle button has its own padding); do not change the 16 points to chase that alignment.
2. **Collapsed group holding the open session.** When a group is collapsed and its members include the open session, its
   header draws a 3-point bar in the accent fill color (`accent_fill`) down its left edge, the way the mockup the
   maintainer approved shows it. It looks like the marked-row marker; share that marker's width constant or element
   rather than copying it. The header's hover help and its accessible text add "Holds the open session: ⟨title⟩", with
   the title as host text (escaped as all host text is). Nothing else about collapse changes: collapse still hides every
   member row, and opening a member (from the quick switcher, a launch, or anywhere else) never expands a group, because
   collapse is shared state and expanding it would change it for every window.
3. **Drag handle.** Every row that can move gets the grip that pinned rows have today, revealed on hover or focus as
   today, at the row's left edge. Pressing a row's body keeps meaning open, and Shift-click keeps meaning mark; the drag
   starts only from the grip. The grip's accessible name says it moves the session.
4. **Drop targets for an unpinned session.** The destination is a whole group: while the dragged session is over a
   group's header or its visible members, the header and members are outlined and tinted together (accent outline, the
   marked-row tint), as in the approved mockup, and no insertion line is drawn, because an unpinned member's place in a
   group follows the list's sort. Dropping there moves the session into that group. The ungrouped rows (not the global
   pins) are the "remove from group" target. Dropping on the session's own group does nothing. A collapsed group is a
   target through its header and stays collapsed (no opening on hover). Global pins are not a target for an unpinned
   session: dragging never pins or unpins a session. The outline and tint are drawn per row (header and each visible
   member), since the list is virtualized.
5. **Pinned sessions.** A pinned session keeps every drop target it has today, with today's meaning: the insertion lines
   among pins in each pin area, including the global pins (which is how a pinned member leaves its group, as today), and
   the existing header targets. The whole-group target is added only where today's pin drag has no target (over a
   group's unpinned members); dropping a pinned session there makes it that group's last pin, through the same request
   today's drop into an empty pin area makes (the membership write with a pin source, which appends the pin last and
   keeps SPEC.md's stale-area refusal). Where an existing pin target and the whole-group target would overlap, the
   existing one wins; log the regions you settled on. Pinned status never changes by dragging. The ungrouped rows and
   the strip in criterion 6 are not targets for a single pinned session.
6. **No ungrouped rows.** When an unpinned member (or a multi-session drag, criterion 7) is dragged and no ungrouped
   rows exist to drop near (global pins do not count), a "Drop here to remove from group" strip (dashed accent outline)
   appears below the last group for the length of the drag, and dropping on it removes the sessions from their groups.
   The frame learns that such a drag is in progress to add the strip.
7. **Marked sessions (the Finder rule).** Dragging the grip of a marked session moves every marked session; dragging an
   unmarked session moves only it. Marks are left as they are. The lifted row shows a "+N" badge, N being the other
   marked sessions that come along. Any drag carrying more than one session has only whole-group targets and "remove
   from group" (the ungrouped rows or the strip), never a position among pins, whatever mix of pinned and unpinned
   sessions it carries; this avoids a positional multi-pin move no helm request makes. Pinned sessions in it keep their
   pin status and go last among the destination's pins. The marked-rows menu's Move to group (`apply_metadata`) also
   clears marks, moves focus and checks against what the menu drew, so a drop cannot call it as it stands: extract its
   per-target loop (the skip check, the refused-row notice and `move_member`) into one shared function that both the
   menu and the drop call, so they skip the same rows and report refusals the same way, and the drop leaves marks alone.
8. **Order hold.** In the window that completes a drop, a moved session appears in its new group at once, as cross-area
   pin moves already do (`Hold::crossed`), instead of sitting where it was until the hold ends, so the drop does not
   look failed. An unpinned session lands directly after the destination's last pin (at the top of its unpinned rows),
   never above its pins, so the group's separator, pin grips and Move up/down stay intact. The shared function in
   criterion 7 reports completion so marked drops get this too; if that cannot be done without a new mechanism, limit
   criterion 8 to single-session drops and log it. Rows that the hold rules leave without a grip today (pins made during
   the hold) keep that rule.
9. **Edge auto-scroll.** While a drag is near the session list's top or bottom edge, the list scrolls, so a group off
   screen can be reached. Built as the stack's last PR.
10. **Specs.** SPEC.md's session list sections (the group header and collapse paragraph around "Collapsing hides all of
    the group's rows", the grip and drag paragraphs around "Hovering an eligible pinned row reveals a grip", the order
    hold paragraphs, and the separator paragraph) describe the new behavior; SPEC_impl.md's matching passages (the frame
    projection, "One grip gesture") follow. Each PR changes the passages its behavior touches.
11. **Tests.** Unit tests in the session list's existing test modules (`session_list/groups_tests.rs`,
    `session_list/pin_drag_tests.rs`, `session_list/pins_tests.rs` and their helpers) cover each criterion. A golden
    scene for groups (none exists today) shows an expanded group with indented members and its guide line, and a
    collapsed group holding the open session. The GPUI end-to-end smoke (`scripts/gui-e2e-smoke.py`) already drags a pin
    grip; add one step that drags an unpinned session into a group and checks the helm lists it there.
12. **No duplicated drag or ordering code.** The pin drag (`session_list/pin_drag.rs`) is generalized to carry the new
    drop targets; there is never a second drag gesture or a second ordering path. This is the maintainer's standing rule
    from the session-groups plan (its criterion 15) and applies here.
13. Changelog fragments per root `AGENTS.md`, "Changelog fragments", one per user-visible PR, describing what that PR
    built.
14. The stack's last code PR removes both TODO.md entries.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, where they approved mockups in the app's dark palette
showing exactly criteria 1 to 7:

- **One plan, built in order:** indent, then the collapsed-group mark, then dragging.
- **Collapsed look: the blue bar on the header's left edge** (option B). Rejected: the whole header taking the open
  row's blue fill; keeping the open row visible under a collapsed header (it would change what collapse means and
  complicate marks, drag and Move up/down); expanding a group when a member opens (collapse is shared).
- **Indent: 16 points with a thin guide line.** Rejected: no guide line.
- **Drag from the grip only,** revealed on hover as with pins today. Rejected: dragging the row body, which would
  collide with click-to-open and with the separate Near term entry "Shift+drag across sidebar rows should mark them
  too".
- **Drop targets as in criteria 4 to 6,** including the "Drop here to remove from group" strip.
- **Marked sessions follow the Finder rule** (criterion 7). The maintainer asked why the planner had first recommended
  moving only the grabbed row; there was no strong reason, and they chose the Finder rule.
- **Edge auto-scroll is in scope,** as the last PR.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the two TODO entries and the Decisions above.

**Binding repository constraints:** SPEC.md and SPEC_impl.md as named in criterion 10; root `AGENTS.md`, "Code rules",
"Changelog fragments", "Conventional Commits", "Tests and validation"; `.agents/test-authoring.md`; the session-groups
plan's no-duplication rule (criterion 12), recorded in that plan's file at its landing
(`git log --format=%H -n 1 --diff-filter=D -- plans/queue/session-groups.md` finds the commit that removed it; the file
is in its parent).

**Planner proposals** (from reading the code at 04631d86; follow the code where it differs, logged as a DECISION):

- The layout lives in `crates/gui/farhelm-gui/src/session_list/groups.rs` (`Layout`, `Frame::new`, `Header`,
  `draw_header`), rows in `session_list/row.rs` with `components/list_row.rs` and `theme.rs`'s `metrics::ROW_PAD`, and
  the drag in `session_list/pin_drag.rs` (`Area`, `Band`, `PinDrag::target`, `drop_in_band`, `send_drop`, `paint`).
  `Frame::new` already sees every member of a collapsed group in its block, so the "holds the open session" flag is a
  cheap addition there once the frame is given the selection.
- Indent by passing the row's band or an indent into `row::row` / `ListRow`, never by a second row-drawing path.
- The helm already moves unpinned rows into groups (`PUT /api/sessions/{id}/group`, `SessionGroupMembership`), which the
  row menu's Move to group uses. No protocol change is needed: the client already cancels a drag whose row changed area
  (`pin_drag_frame` compares `area_of`), and menu moves carry no stale-source check either.
- `hold.rs`'s `Hold::crossed` acts only on original pins today; extend it to unpinned rows for criterion 8.
- The keyboard path already exists (the Move to group submenu in the row and marked-rows menus); nothing new is needed
  for keyboard users.

## Outline

1. `plan/session-group-follow-ups/01-indent-members`: criterion 1, the golden scene's expanded half, SPEC.md. `feat:`
   (or `style:` by its effect), with a fragment.
2. `plan/session-group-follow-ups/02-collapsed-open-mark`: criterion 2, the golden scene's collapsed half, SPEC.md.
   `feat:`, with a fragment.
3. `plan/session-group-follow-ups/03-generalize-drag`, only if real restructuring is needed: `refactor:` with no
   behavior change, reshaping `pin_drag.rs`'s areas and bands and extracting criterion 7's shared move function, proven
   by the unchanged `pin_drag_tests.rs` and `pins_tests.rs` passing. `pin_drag.rs` is already generic over areas and
   bands; if the new targets fit without restructuring, fold this into PR 4 rather than adding variants nothing uses yet
   (clippy rejects dead code). Log which.
4. `plan/session-group-follow-ups/04-drag-into-groups`: criteria 3 to 6 and 8 for a single session, SPEC.md and
   SPEC_impl.md, the end-to-end smoke step. `feat:`, with a fragment.
5. `plan/session-group-follow-ups/05-drag-marked`: criterion 7. `feat:`, with a fragment (or folded into PR 4's fragment
   while both are unreleased, per root `AGENTS.md`).
6. `plan/session-group-follow-ups/06-drag-auto-scroll`: criterion 9. `feat:`, with a fragment. Removes both TODO.md
   entries.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check; clippy with `--all-targets` on
`farhelm-gui`; the GUI crate's unit tests for the session list through `python3 scripts/record-test-run.py`; the GPUI
goldens (`python3 scripts/check-gui-goldens.py`, with `--update` and `python3 scripts/gui-goldens.py publish` for the
new scene, looking at every changed picture before committing the manifest); the GPUI end-to-end smoke when PR 4 adds
its step; and `python3 releasing/check-changelog.py format`.

## Fallbacks

- If `pin_drag.rs` cannot be generalized without changing pin behavior, that is a departure: run the scope reassessment
  before building anything beside it. A second drag gesture is not an acceptable outcome.
- If a marked drop needs a helm request the marked-rows menu does not already make, that is a departure: run the scope
  reassessment.
- If edge auto-scroll needs list scrolling support GPUI's list does not offer, build it in the session list's own scroll
  handling; if that grows past a small change, deliver PRs 1 to 5, record the obstacle in the report, and leave
  auto-scroll undone rather than block. In that case the last PR does not remove the "Drag sessions into groups" entry:
  it narrows it to the edge auto-scroll that remains, so the maintainer's decision is not lost.

Decisions to log: the indent mechanism, how the guide line is drawn, the drop-target geometry, how "+N" and mixed marked
sets are handled, every test and golden changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-session-group-follow-ups-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-session-group-follow-ups-watchdog.json` beside it. Resolve
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
  `plan/session-group-follow-ups/<nn>-<short-name>`, as the outline names them.
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

In the session list, you can now drag any session into a group, out of one, or between groups, using the grip that
appears when you hover over a row. Group members are now indented under their group, and a collapsed group is
highlighted when it holds the open session.

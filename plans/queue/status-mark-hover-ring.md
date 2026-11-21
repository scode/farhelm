# No hover ring on status icons

Written against main at 3617ee450 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **No box around a session row's status icons on hover.** the "box" that appears around the idle time and the agent
> type/permission mode should go away. (Screenshot: hovering a session row in the sidebar draws a rectangle around the
> row's agent mark and permission-mode icon.) hover is fine, but it should not act as if it's a clickable
> button/something selectable, which is basically what that appearance of the rectangle on hover is showing Plan:
> `plans/queue/status-mark-hover-ring.md`.

### Where it stands (from planning research at main 3617ee450)

The box is the **tooltip target ring**, which the maintainer chose on 2026-11-07 (planning page of
`row-hovers-beside-list`, option B, "a notch plus a ring") and which the 2026-11-19 hover audit (#4288, #4290) spread to
group headers, attention rows, Hosts rows, the host rail and in-window lists. It is not button styling, a focus ring or
a click target. An earlier planning page (2026-11-20) asked which targets lose it and was deferred; the maintainer has
now answered (Decisions below).

- The layer manager draws the ring, not the icon: `PlacedBeside::new` builds it when the help's placement is `Beside`,
  `Rail` or `List` (`crates/gui/farhelm-gui/src/layers/beside.rs`, about lines 446-456) and paints a 1-pixel quad just
  inside the target (about 722-740, debug selector `tooltip-ring`). Cues come from `BesideCues`
  (`crates/gui/farhelm-gui/src/layers/tooltip.rs`, about 259-270). Tests read it through `ShownTooltip::ring()` (about
  line 447), which is `None` when nothing was painted. Menu help is already beside with a notch and no ring
  (`HelpPlacement::Menu`), so a ringless beside card is an existing look.
- `HoverHelp::beside()` / `beside_rail()` / `beside_list()` / `ring_on_fill()` / `ring_radius()` all turn the ring on
  (`crates/gui/farhelm-gui/src/components/mod.rs`, about 224-293). Session rows route every hover through
  `HelpPlace::Row { selected }` (`crates/gui/farhelm-gui/src/session_list/row_marks.rs`, about 351-381).
- `StatusMark` (`crates/gui/farhelm-gui/src/components/status_mark.rs`) is "not a control, so not a tab stop, and a
  click on it does nothing"; it carries its own screen-reader name and description. All the targets that lose the ring
  render through it: session rows' host or environment mark, agent track (agent mark plus permission shield, one mark),
  age, Pinned, compact ended mark, stale clock or cause, Suspended and Deleting; Hosts rows' host mark and phase; the
  group header's attention dot; the host rail's "?".
- SPEC.md, "Session list" (about lines 2751-2754: "a thin accent ring identifies that target while its help shows"),
  about 2796-2798 ("A notch and a thin ring identify the target in both cases") and about 2805-2808 ("A notch and ring
  identify the control"); SPEC_impl.md about 950-953 ("Sidebar, rail and list help also draws a 1-pixel ring …") and
  about 938 ("List help shares the sidebar's notch and ring").

### Acceptance criteria

1. Hovering any status mark (anything rendered through `StatusMark`, in session rows, Hosts rows, group headers, the
   host rail or elsewhere) shows its help exactly as today (placement beside the sidebar, the notch, the delay, the
   wording) but draws no ring around the mark, on unselected and selected rows alike.
2. Real controls keep the ring exactly as today: the status dot, the mute bell, the notification bell, the pin grip, the
   row actions button, rail entries, group headers, Hosts rows, list rows, and the whole row while its summary card
   shows.
3. The rule is structural: a status mark cannot ring, including one added later; it is not decided per call site.
4. SPEC.md (about 2751-2754 and 2796-2798) and SPEC_impl.md (about 950-953) say that the ring identifies a control whose
   help shows, and that status marks show the notch only. The in-window list paragraphs (SPEC.md about 2805-2808,
   SPEC_impl.md about 938) stay as they are: no status mark uses list placement. The doc comments in `status_mark.rs`,
   `row_marks.rs` (`HelpPlace`) and the session row module that say every row hover gets a ring are corrected.
5. Tests: status marks assert beside placement, a notch and `ring().is_none()`; controls keep their ring assertions; a
   component test for a status mark's beside help without a ring. The changed goldens are updated and visually reviewed.
6. The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Scope:** every status icon everywhere loses the box (session rows, Hosts rows' marks, the group attention dot, the
  rail "?"); one rule, "the box marks something you can press". Not only session rows, and not controls.
- **The whole-row box while a row's summary card shows stays**: the row is clickable.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort, plus the mandatory Opus visual review
  (see "Visual review").
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- SPEC.md and SPEC_impl.md text quoted above, which this plan changes where acceptance criterion 4 says.
- `.agents/test-authoring.md` for every test added or changed; the GPUI goldens check (`.agents/validation.md`).
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and its "Instructions by
  path" for every path you touch.

**Planner proposals:**

- Generalize `BesideCues` just far enough: one cue that turns the ring off (default keeps today's ring, so every other
  caller is untouched), honored in `PlacedBeside::new`; notch, side, centering and fallback unchanged.
- A `HoverHelp::without_ring()` private to the components module (a no-op for placements that never ring), applied once
  in `StatusMark::render` before it registers its help. Keeping it out of reach of other modules is what keeps the rule
  structural.
- Pitfalls found in planning review: (a) do not build the helper on the internal `beside_cues()` set-up, or the host
  rail "?" silently switches from rail placement to ordinary sidebar placement and its card moves; handle the rail case
  directly. (b) The cue struct's defaults come from `Default`, and a plain boolean defaults to false, which would strip
  the ring from every control: name the field so its default keeps the ring (for example `no_ring`), or write the
  default by hand; the one hand-built instance (`goldens/foundations.rs`, about line 603) needs the field too. (c) The
  row-mark tests' shared helpers always expect a ring, and the test's docstring uses the ring to tell a mark's help from
  the row's summary card; splitting the test needs a ringless variant of those helpers, not just a flipped assertion.
  (d) The notification bell does not route through `HelpPlace::Row`; it is a control and keeps its ring regardless.
- Update the golden `session-row-hover-beside` (`crates/gui/farhelm-gui/src/goldens/mod.rs`, setup in
  `goldens/session_list.rs`, which hovers the idle age) and its title in `docs/gpui/screenshot-scenes.json` ("…and the
  accent ring on it"); `session-summary-card` and `host-rail-hover-beside` should stay. Find any other changed golden
  with the golden changes page.

## Outline

1. `plan/status-mark-hover-ring/01-status-marks-no-ring`: the cue, the `StatusMark` rule, the doc comments, SPEC.md and
   SPEC_impl.md, the tests (split `every_row_mark_shows_its_help_beside_the_sidebar_with_its_ring` in
   `session_list/summary_tests.rs` into marks and controls; invert the ring half of
   `a_compact_row_shows_the_same_card_with_its_hidden_folder`; add the component test), the goldens, a changelog
   fragment, and the TODO entry's removal. `style:` or `fix:`, your call; either way a fragment (`kind: changed`).

## Fallbacks

- Planning review found every status icon with beside help renders through `StatusMark`. If one turns out not to, route
  it through `StatusMark` rather than widening the helper's visibility, and log it.
- `docs/gpui/parity/sidebar-and-session-view.md` names the old test and golden; it is a migration ledger, so updating it
  is your call (log it).
- A golden that changes for a control (not a status mark) is a red flag: investigate before accepting it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-status-mark-hover-ring-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-status-mark-hover-ring-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  `plan/status-mark-hover-ring/01-status-marks-no-ring`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("No box around a session row's status icons on
  hover"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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

### Visual review

The maintainer also demands a visual review for every PR that changes what the desktop app draws: a fresh-context
`claude-opus-5-5` agent at `high` effort, launched natively where your harness can (otherwise through
`$scode-harness-shellout`), separate from the reviewer above. Give it headless renders of every changed scene in both
the light and the dark appearance (the GUI golden scenes, regenerated with
`python3 -B scripts/check-gui-goldens.py --update`, and the golden changes page that run writes), the design this file
describes (its mockup text and wording are the agreed design), and the PR's diff. It must be satisfied that the rendered
result matches the intent of the agreed design; loop until it reports no mismatch. A mismatch with the agreed design is
binding and is fixed; any other observation it makes is an input, handled like the reviewer's findings above. Do not
write a launch command here or in the log.

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
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate and,
where it changes what the app draws, its visual review, and the last removing the TODO.md entry this plan covers. Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.

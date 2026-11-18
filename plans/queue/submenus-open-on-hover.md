# Submenus open on hover

Written against main at 04631d86 on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan's PR writes the new rule into SPEC.md.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Open submenus on hover, as a general rule.** The shared submenu component
> (`crates/gui/farhelm-gui/src/components/submenu.rs`, first used by mute-sessions and reused by session-groups for Mute
> and Move to group) opens a submenu only on click, Enter or Right; while it is open its parent item loses its
> highlight. Make submenus open on hover, as most desktop submenus do, keep the parent highlighted while its submenu is
> open, and state hover-to-open as the general UI rule for submenus in SPEC.md, implemented once in the shared component
> so every submenu follows it. Recorded by the maintainer on 2026-11-18 from the mute-sessions review.

### Acceptance criteria

1. Resting the pointer on a menu item that has a submenu opens the submenu at once (no delay), beside the parent item,
   as Right and Enter open it today.
2. When hovering opens a submenu, the parent item keeps the menu's highlight and nothing in the submenu is highlighted.
   Moving the pointer into the submenu, or pressing Right, moves the highlight into it; Enter on the highlighted parent
   still opens the submenu and moves into it, as today. This is how macOS and Windows behave, and it keeps Enter acting
   on the item the user sees highlighted.
3. While a submenu is open, its parent item stays highlighted (today it loses its highlight because the highlight is
   focus). The keyboard focus ring stays only on the focused item.
4. Moving the pointer onto another item of the parent menu closes the open submenu at once, and opens that item's own
   submenu if it has one. Hovering parent-menu items works while a child submenu is open (today pointer hover moves the
   highlight only inside the top layer). This applies only to a menu's own submenus: hovering a menu never closes a
   dialog, question or other layer opened from it.
5. Clicking a parent item whose submenu is already open does nothing; the submenu stays open. Clicking a parent item
   whose submenu is closed (the pointer rested on it, then Escape closed the submenu, say) opens it and moves into it,
   as a click does today. Enter and Right on a parent whose submenu hover opened move into the submenu (criterion 2),
   while a click on it does nothing, so the one handler those three share today splits.
6. Hover never runs a command, and SPEC.md's 2026-10-17 race rule after the submenu paragraph still holds.
7. It is implemented once, in the shared submenu component (`crates/gui/farhelm-gui/src/components/submenu.rs`) and the
   menu item and layer code it relies on, so every submenu follows it: today Mute in the row menu, Mute in the
   marked-rows menu, and Move to group in both.
8. SPEC.md's submenu paragraph ("A menu item with a submenu opens it beside the parent item…", in "Things changing
   underneath") states hover-to-open as the general rule for submenus, with the parent-highlight exception to the
   one-highlight rule. SPEC_impl.md's layer-manager paragraph, the layers module docs and the GUI ownership doc
   (`docs/gpui/foundation-ownership.md`) say what changed in the layers code.
9. Tests: the component's own test and the session list's menu tests (`session_list/menu_tests.rs`, whose `lit()` checks
   the painted highlight, and its one-highlight tests) cover criteria 1 to 5. The hover tests move the pointer onto the
   item; the existing test click helper sends no pointer move, so a click-only test never exercises hover.
10. A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, which showed a mockup of a hovered Mute item with its
submenu open beside it:

- **The parent stays highlighted and nothing in the submenu is,** until the pointer moves in or Right is pressed.
  Rejected: highlighting the submenu's first item on hover.
- **Open and close immediately,** with no delay. The planner had recommended about 0.3 seconds, the delay tooltips use,
  so a pointer passing over an item does not flicker its submenu and a diagonal move toward a submenu does not close it
  by crossing another item. The maintainer chose immediate. Do not add a delay or a "safe triangle"; if immediate
  closing makes a submenu hard to reach in practice, say so in the report's things you should know.
- **A click on an open parent does nothing.** Rejected: toggling it closed.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** SPEC.md's submenu and one-highlight paragraphs (criterion 8); root `AGENTS.md`,
"Code rules", "Changelog fragments", "Conventional Commits", "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals** (from reading the code at 04631d86; follow the code where it differs, logged as a DECISION):

- `submenu()` opens the child only from `on_click` today; Right arrives through `components/menu_item.rs` (around line
  367) via `submenu_handler`. The child opens with `Popover::menu(panel, …, anchor, start)`, which focuses its first
  item. In `menu_item.rs` the highlight is literally focus (`lit = enabled && focus.is_focused(window)`); the
  `submenu_expanded` flag drives only `aria_expanded` and the chevron.
- Pointer hover moves focus only inside the top layer (`focus::highlight`, `focus.rs` around line 1581).
- Criterion 2 needs a child menu opened without taking focus, and the layers code already has that: a floating popover
  opens the same kind of layer without moving focus, and every focus rule already handles a layer that does not hold
  focus. Do not make the child "take" focus while focus stays on the parent: that would send Down, Up, Tab, Home and End
  in the parent menu into the submenu and confuse focus recovery. The smaller design: hover opens the child without
  focus (as a floating popover, or through a new builder on the menu preset; a close call, yours to make and log), and
  one new layers function hands focus to it when the pointer moves in or on Right or Enter. Neither is a new popover
  kind.
- Draw the open-submenu parent with the same accent fill as the highlight, driven by `submenu_expanded`.

## Outline

One PR: `plan/submenus-open-on-hover/01-hover-opens-submenus`, `feat:`, with SPEC.md, SPEC_impl.md, tests and the
fragment. If the layers function that hands focus to an unfocused child turns out to be a sizeable change of its own,
split it out first as `plan/submenus-open-on-hover/01-hand-focus-to-child` (no behavior change) and make the feature
PR 02.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-gui`, the GUI crate's component, menu and layer unit tests through `python3 scripts/record-test-run.py`, the
GPUI goldens if a drawn menu state changes, the GPUI end-to-end smoke (it opens row menus), and
`python3 releasing/check-changelog.py format`.

## Fallbacks

- If, contrary to the planner's reading, the layers code cannot open a child menu without focus except by a new popover
  kind, that is a departure: run the scope reassessment.
- If immediate closing makes a submenu unreachable in tests (a pointer path that must cross another item), keep the
  maintainer's immediate rule and record it in the report rather than adding a delay.

Decisions to log: how the unfocused open works, how the parent's highlight is drawn, every test changed and why, and
every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-submenus-open-on-hover-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-submenus-open-on-hover-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  `plan/submenus-open-on-hover/<nn>-<short-name>`, as the outline names them.
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

Submenus, such as Mute and Move to group in a session's menu, now open when you hover over them, as in other desktop
apps.

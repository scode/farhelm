# Show keyboard shortcuts in pop-up menus

Written against main at 4638a5334 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Implement the TODO.md entry under Near term → Trivial, **Show key bindings in pop-up menus.** The maintainer's words:
"key bindings should be shown in pop-up menus for discoverability (so eg. "rename" shows cmd-l)". In the desktop app, a
menu item whose action has a keyboard shortcut shows that shortcut, so people learn the keys from the menus they already
use.

### Where it stands (from planning, at main 4638a5334)

Line numbers are approximate; paths are under `crates/gui/farhelm-gui/src/`.

- Menus are Farhelm's own components: `components/menu_item.rs` (each item is a 14px icon, a label and an 11px
  description line underneath; no shortcut slot today; the submenu chevron is pushed to the right end of the label line
  by a spacer, about lines 342-354), `components/submenu.rs`, `components/menu_header.rs`, and the menu layer in
  `layers.rs`. Most menu panels are 246px wide.
- Only three existing menu items have a keyboard shortcut:
  - the session menu's **Rename**: ⌘L on the Mac, Ctrl+Shift+L on Linux;
  - the session menu's **Delete**: ⌘W on the Mac, Ctrl+Shift+Backspace on Linux;
  - the Help menu's **Keyboard shortcuts**: ⌘/ on the Mac, Ctrl+Shift+/ on Linux.

  The session menu is in `session_list/menu.rs` (items about lines 1266-1283); the Help menu in `help_menu.rs` (about
  line 205, the function that orders its items). Both session shortcuts act on the **open** session only. ⌘W runs the
  open session's header Delete (SPEC.md, about line 3688), which always asks before deleting; the row menu's Delete asks
  only sometimes, and has variants for environment sessions (including one already being deleted) and for retired rows.
  The open session's Rename item already has a tooltip ending in "(⌘L)" (`session_list/menu.rs`, about lines 905-909).
- Shortcuts with no menu item at all (New ⌘N, the quick switcher ⌘K, the template list ⌘J, ⚡ launch ⌘P, the sequence
  prefix ⌘X) are out of scope.
- Chords are defined in `actions.rs` (bindings about lines 400-475; per-platform constants such as
  `RENAME_SESSION_CHORD`, about lines 168-178). Display strings already exist for these chords but are scattered:
  `actions.rs` (about lines 162-185, "⌘W" and "⌘L" and their Linux forms, used by the Rename and Delete tooltips), and
  `shortcuts.rs` (`Chord::label`, about line 89). Keyboard shortcuts itself is matched by physical key position
  (`PHYSICAL_KEYS`), not a keymap binding, and its chord's text exists only through a private label function in the
  Keyboard shortcuts table; there is no constant for it yet. GPUI's own `keystroke_text_for` does not fit: it prints
  "ctrl-shift-L" on Linux, depends on focus, and cannot see the physical-key shortcuts.
- Shortcuts already show as small key chips in the Keyboard shortcuts dialog (`shortcuts::chip`, about line 404) and in
  the ⌘J template list.
- SPEC.md (about lines 3702-3705) says where the session shortcuts are shown today: "Delete on the header button, Rename
  only on the open session's row-menu item" (as hover hints). The Keyboard shortcuts dialog is specified about lines
  3754-3765; SPEC_impl.md has "Actions and keymap" (about line 1255).
- GUI screenshot goldens: `help-menu` and `help-menu-announcements-hover` will change. `session-row-menu` opens the menu
  of a row that is not the open session, so it should not change. Others change only if the shared item layout shifts.
  Tests that hold these strings: `session_list/menu_tests.rs` (about line 1087), `session_pane/header_tests.rs` (about
  lines 342-358), `components/tests/menu_item.rs`, `shortcuts/tests.rs`.

### Acceptance criteria

1. A menu item can carry a shortcut, drawn as a key chip at the right end of its label line (the place the submenu arrow
   uses), in the look of the Keyboard shortcuts dialog's chips. Items without one look exactly as before.
2. The open session's menu shows ⌘L on Rename and ⌘W on Delete (Ctrl+Shift+L and Ctrl+Shift+Backspace on Linux). The
   menu of any other session shows no chips, because those keys act on the open session. Delete's chip shows only for
   sessions where the menu's Delete and ⌘W do the same thing (tests pin which kinds of session those are); where they
   differ, Delete shows no chip. With the chip showing, the Rename item's tooltip drops its "(⌘L)", which would only
   repeat it; the header Delete button's "(⌘W)" tooltip is not a menu item and stays.
3. The Help menu's Keyboard shortcuts item shows ⌘/ (Ctrl+Shift+/ on Linux).
4. The text in each chip comes from the same per-platform display strings the app already uses for these chords (the
   tooltips and the Keyboard shortcuts dialog), so a menu chip and its tooltip never disagree.
5. Linux shows the full form ("Ctrl+Shift+L"), as tooltips and the Keyboard shortcuts dialog do. Chips fit the 246px
   panels without clipping the item's label.
6. SPEC.md's sentence about where the session shortcuts are shown is updated to include the menu chips. Tests cover the
   open-session and other-session cases, and the changed goldens are re-captured (adding one that pictures the open
   session's menu if no existing golden shows a chip on a session menu).

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page, where three mockups were shown:

- **Placement:** a key chip right-aligned on the label line. Rejected: quiet text right after the label, and the
  shortcut in the description line.
- **Which session menus:** only the open session's menu, where the key does exactly what the item does. Rejected: every
  row's menu.
- **Scope:** show keys only on items that already exist; add no menu items for shortcut-only commands.
- **Linux form:** the full "Ctrl+Shift+L", as shown in the mockup's caption and not contested.
- **Delete's chip and the Rename tooltip** (planner decisions shown to the maintainer on the follow-up page, after the
  planning review raised them): Delete's chip only where row Delete and ⌘W match; the Rename tooltip drops "(⌘L)".
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort with the general review charter.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry quoted above and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md", and
  its "Instructions by path" for every path you touch.
- `.agents/validation.md` for the GUI test selections and the screenshot goldens (re-capturing them with
  `scripts/check-gui-goldens.py --update`, inspecting every changed picture, and publishing them with
  `python3 scripts/gui-goldens.py publish` before any rebase, as it describes).
- `.agents/test-authoring.md` for every test added or changed.
- SPEC.md's keyboard shortcut paragraphs (about lines 3702-3765).

**Planner proposals:**

- Reuse the existing per-platform display strings for Rename and Delete, and add one constant for Keyboard shortcuts'
  chord beside them in `actions.rs`, with a test that it agrees with the Keyboard shortcuts table's label, rather than
  building a general keymap-to-text layer, and reuse the Keyboard shortcuts dialog's chip look rather than adding a
  second chip style. If the dialog's chip cannot be reused as it stands, share its styling with the smallest change that
  serves both.
- Leave the other scattered chord strings (template keys, text size) alone; unifying them is not part of this entry.

## Outline

1. `plan/menu-shortcut-hints/01-menu-shortcut-chips`: `feat: show keyboard shortcuts in menus`. Add the optional
   shortcut chip to the menu item component, set it on the open session's Rename and Delete and on Help → Keyboard
   shortcuts, update SPEC.md, add the tests, re-capture and publish the goldens, leave a changelog fragment
   (`kind: added`), and remove the TODO.md entry ("Show key bindings in pop-up menus.").

## Fallbacks

- If a chip would clip the Linux "Ctrl+Shift+Backspace" against Delete's label in a 246px panel, let the label line wrap
  or shrink the chip text rather than widening every panel; log the choice as a DECISION.
- If the session menu cannot tell whether it belongs to the open session without new plumbing, use whatever already
  decides the open session (the row's selection state) rather than adding new state; log it.
- Decisions to log: how the chip's text is sourced, any golden whose change you judged intended, and every review
  finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-menu-shortcut-hints-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-menu-shortcut-hints-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what the PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: `plan/menu-shortcut-hints/01-menu-shortcut-chips`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built (`plans/.agents/executing.md`, "Executing one plan", step 8).
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
working log with the true value; never edit this plan file. If current code already does what an entry asks, or the code
it targets no longer exists, record that in the working log and the report, remove the entry in the plan's PR, and build
only what is still missing.

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
meeting the acceptance criteria above, each having passed its review gate, with the last one removing the TODO.md
entries this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through
the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.

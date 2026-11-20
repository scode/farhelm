# Let the Templates dialog close: no needless save question, and one you can see

Written against main at 3fd37f471 on 2026-11-20. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Templates dialog can't be closed after scrolling to the bottom.** i can trivially get into a state that requires me
> to restart farhelm. open templates, pick one template, scroll to the bottom. now hitting esc just changes some scroll
> state, and hitting esc again changes it back, so hitting esc esc esc esc just toggle back and forth and never exits
> the menu. clicking the "X" at the top right also doesn't exit the kmenu and i'm hard stuck and have to restart farhelm

### Where it stands (from planning, at main 3fd37f471)

The planner found the cause by reading the code; nobody has reproduced it in the app.

- The dialog is `crates/gui/farhelm-gui/src/templates/dialog.rs`. Escape (`Dismiss`) and the header Close both reach
  `TemplatesDialog::dismissal`, which, while the editor's draft is unsaved (`Editor::dirty`, `draft != baseline` in
  `templates/editor.rs`), calls `ask_leave(Departure::Close)` and stays.
- `ask_leave` asks "Save these changes before leaving?" (Keep editing, Discard, Save) through `prompt::ask_inline`,
  drawn in place of the editor's action footer at the foot of the scrolling editor, in the plain look (not the loud
  one). It first focuses the name field at the top, then the prompt focuses Save at the bottom, scrolling each into
  view.
- Escape inside the inline question means Keep editing (Cancel), which returns focus to the name field and scrolls back
  up. The next Escape comes from the name field, reaches `dismissal` again, and asks again. So Escape alternates between
  "ask, scroll to the bottom" and "cancel, scroll to the top" forever, which is what the maintainer saw as scroll state
  toggling. The header Close, pressed while the question is up, replaces the question with an identical one, so nothing
  visibly changes. Only Discard or Save leaves, and the plain question at the foot reads as ordinary text.
- Why an untouched template asks at all: `Draft::from_template` (`templates/fields.rs`) fills in the launcher switch
  that a template saved before switches existed implies, and drops an old workspace-trust field, while the editor's
  baseline keeps the raw stored fields (`Editor::open`). Either difference makes a freshly opened template unsaved.
  SPEC.md ("Launch templates": "opening one infers its switch in an unsaved draft, and saving stores that switch") and
  `templates/editor_tests.rs` require that today.
- The Settings recipe editor asks the same leave question inline, in place of its answers at the bottom of the scrolling
  panel (`crates/gui/farhelm-gui/src/settings/recipes/editor/leaving.rs`), for Back, Cancel and closing the hosting
  dialog (Settings, or Add host, whose answers sit in its pinned footer). Its delete confirmation is inline too. Nobody
  has reported a problem there; the maintainer chose to change its leave question as well so the two editors match.
- The centered question (`prompt::ask`, a layer with a veil, used by the session header's Stop and Delete questions)
  supports a relabelled Cancel (`Question::cancel_answer`) and extra answers (`Question::alternative`), which is what
  Keep editing and Discard need.

### Acceptance criteria

1. **Templates: leaving asks only about the user's own edits.** Every departure from the template editor (another row,
   "+ New template", Duplicate, the header Close, Escape) asks only when the draft differs from what the editor showed
   when the template was opened or last saved. When the only differences from the stored template are the ones Farhelm
   made on opening (the inferred launcher switch and the dropped workspace-trust field), the departure goes ahead at
   once and the stored template is left exactly as it was. While the template is open, everything else stays as today:
   the editor shows the inferred switch, "Unsaved changes" and Save follow today's rule against the stored template, and
   Save stores the switch. An edit the user undoes back to what was opened no longer asks.
2. **Templates: the question shows over the dialog.** When leaving does ask, "Save these changes before leaving?" with
   Keep editing, Discard and Save, in that reading order and with focus starting on Save, is the centered question over
   the Templates dialog with its veil, not the inline band at the editor's foot. Escape in it, and Keep editing, close
   it with the draft kept and focus back in the editor (the name field, where today's Keep editing returns it). Discard
   goes where the user was heading. Save saves, and goes there only once the save succeeds; a refused save keeps the
   dialog and the draft, with the helm's reason in the status line, as today. The rules for writes in flight stay as
   today.
3. **Settings recipe editor: the same.** Its leave question (Back, Cancel, closing Settings with Escape or a close
   control, and the same in Add host) is the centered question over the hosting dialog with the same answers, order,
   starting focus and Escape rule, keeping its existing variants: the question offers Keep editing and Save only while a
   save is out; that save's success completes the close even while the question is unanswered, taking the question with
   it; and that save's refusal takes the question away, with focus on Save and the helm's words in the editor (today
   pinned by `a_refused_save_takes_an_open_question_away` in `settings/recipe_leave_tests.rs`), so the question never
   stays up offering only a Save that resends what the helm refused. Its delete confirmation stays inline, as today, and
   asking the leave question still takes an open delete confirmation away.
4. **Tests.** Regression tests, written per `.agents/test-authoring.md`, pin the reported loop away. In the Templates
   dialog: an untouched template saved without a switch and carrying an old workspace-trust field closes on the first
   Escape, and on the header Close, without asking; an edited template's Escape opens the centered question, Escape in
   it returns to the editor with the draft, and Discard then closes the dialog. Escape on an edited template alternates
   between the question open and closed while the dialog stays, so tests must not expect repeated Escapes to close the
   dialog. In the recipe editor: Escape on an edited draft opens the centered question, and Discard completes the close.
   Existing tests that assert the inline question (`templates/leave_tests.rs`, `settings/recipe_leave_tests.rs`,
   `templates/editor_tests.rs` for the untouched-template rule) change with the behavior rather than being kept
   alongside it. Tests of behavior that only an inline question has, and that a veiled question makes impossible, are
   deleted rather than adapted; the planning review named `save_on_a_draft_no_longer_unsaved_leaves_at_once`,
   `a_new_departure_replaces_the_question`, `the_question_is_scrolled_into_view` and
   `answers_while_a_bolt_write_runs_keep_the_draft` in `templates/leave_tests.rs`. Check each before deleting it.
5. **Docs follow.** SPEC.md "Launch templates" says the inferred switch and the dropped trust field alone do not make
   leaving ask, and that the leave question shows over the dialog. SPEC.md's recipe editor paragraph (the one saying
   "Its actions and leave/delete questions and refusal messages stay visible at the bottom of the scrolling panel")
   keeps the delete question and refusals there and moves the leave question over the dialog. SPEC_impl.md's prompt
   paragraph that names "the template editor's leave question at its foot" as an inline question is updated. The module
   docs of `templates/dialog.rs`, `templates/editor.rs`, `settings/recipes/editor/leaving.rs` and `prompt.rs` that
   describe the inline leave question are rewritten to match, and so are the parity notes that describe it and cite its
   tests by name: `docs/gpui/parity/templates-and-quick-launch.md` and `docs/gpui/parity/settings-and-hosts.md`, where
   moving the question off the old app's inline band is recorded as a new deviation. Check the module docs of
   `hosts/add/tensorlake.rs`, which mention the editor's question.
6. **Screenshot golden.** The one golden that shows a leave question, the `template-leave-question` scene
   (`template_leave_question_setup` and `template_leave_question_scene` in
   `crates/gui/farhelm-gui/src/goldens/templates.rs`), is re-captured as `.agents/validation.md` describes, and the new
   picture is looked at to confirm it shows the centered question. No recipe editor golden shows its leave question.

## Decisions already made

Settled with the maintainer on the 2026-11-20 planning page:

- **Untouched templates:** leaving without asking when the only differences are the ones Farhelm filled in itself. The
  page said: "Either way, the editor still shows the filled-in switch while the template is open, and Save still stores
  it." This changes SPEC.md's "opening one infers its switch in an unsaved draft" sentence.
- **Placement:** "A question over the dialog": a centered question on top of the Templates dialog, with the dialog
  dimmed behind it, like the Stop and Delete questions in a session's header. Escape answers Keep editing, and the
  header Close cannot be clicked while it shows. Picked over a band pinned to the dialog's bottom edge and over
  restyling the inline question in place.
- **Scope:** "Both, so they match": the Settings recipe editor's leave question moves over its dialog too. Its delete
  confirmation was not part of the question and stays inline.
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **Release note:** "No release note needed."
- **No-workhorse mode**, the plans default, stated on the planning page.

Planner choices: two PRs rather than the one the page stated, because the scope answer added the recipe editor after the
page was written; and, in the Templates dialog, Keep editing returning focus to the name field, as today's does. In the
recipe editor it returns to the control that asked, as today's does.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- SPEC.md "Launch templates", the recipe editor paragraphs under "Topology", and "Opening and closing" (Escape's rules
  for dialogs and questions). SPEC_impl.md's prompt paragraphs.
- `.agents/test-authoring.md` for every test added or changed, and `.agents/validation.md` for the GUI tests and the
  screenshot goldens.
- Root `AGENTS.md` "Conventional Commits", "Changelog fragments", "Tests and validation".

**Planner proposals** (verified against the code):

- **Two comparisons in the template editor.** Keep `Editor::dirty` (draft against the raw stored baseline) for "Unsaved
  changes" and Save: SPEC.md greys Save out when nothing is unsaved, and the maintainer wants Save to still store the
  inferred switch. Add a second comparison for what leaving asks about: the draft against the draft as opened or last
  saved. It needs no new stored state: `Draft::from_template(&self.baseline.template())` is already that value
  (`Editor::duplicate` uses it; after a save the baseline is the saved draft, which `from_template` leaves unchanged).
  Storing an `opened` snapshot instead is equally small; choose one, not both, and log it. Use it in `dismissal` and
  `leave`. Name both comparisons clearly and document which one each caller uses and why.
- **Reuse `prompt::ask` as it stands** for both leave questions, with `cancel_answer` for Keep editing, `alternative`
  for Discard (left out while a recipe save is out), Confirm as Save, and `Question::returns_to` naming the Templates
  name field (which costs no focus move before the question opens). Drop today's pre-focus of the name field in
  `ask_leave`: it scrolls the editor to its top, part of the jumping the maintainer saw. The layer manager already
  supports a dismissal hook opening a question over its dialog (host removal and uninstall do it over Settings), closes
  the question with its dialog, and returns focus into the dialog. Planning found no gap in `prompt.rs` or `layers.rs`;
  any change there is a departure that goes through the scope reassessment first.
- **The recipe save's completion needs the window.** Today `Editor::saved` (`settings/recipes/editor/leaving.rs`) asks
  the inline question whether it is open, and the refusal path clears it, without a window. A centered question is a
  layer, which only `layers::is_open` and `layers::close` can query and close, both taking the window, and the save's
  completion runs without one. Keep the question's `LayerId` and reach the window through a stored window handle, as the
  template editor's `Editor::window` and the Recipes section's `window` already do.
- **Drop what the veil makes dead.** Both Save answers check "nothing left to save" because the fields stayed editable
  under the inline question (`templates/dialog.rs` in `ask_leave`, and the recipe editor's Save answer in `leaving.rs`).
  Under a veiled question the draft cannot change, so remove those branches. The template editor's stored inline
  question (`Editor::leave`) and the footer branch that draws it go away; the Templates dialog need not keep the
  question's `LayerId`, since it closes with the dialog. Keep the prompt's inline machinery: the Trash dialog, Settings'
  containers and Tensorlake sandboxes, the launcher's questions, the interrupted card and the recipe delete confirmation
  still use it.

## Outline

Two PRs, in this order:

1. `plan/templates-dialog-close/01-templates-leave` (`fix:`): criteria 1 and 2 for the Templates dialog, their tests,
   the template golden re-capture, the SPEC.md "Launch templates" text, the SPEC_impl.md and `prompt.rs` doc lines that
   name the template question as inline, the Templates module docs and parity notes, and a changelog fragment.
2. `plan/templates-dialog-close/02-recipe-leave` (`fix:`): criterion 3 for the recipe editor, its tests, the SPEC.md
   recipe editor paragraph, its module docs and parity notes, the fragment from PR 1 extended to cover it (an edit of
   the unreleased fragment, not a second entry, per root `AGENTS.md`, "Changelog fragments"), and the TODO.md entry
   removed. Doc changes land once, in the PR whose behavior they describe, not spread over both.

## Fallbacks

- If the recipe editor's save-out variant turns out not to fit on a centered question after all, run the scope
  reassessment before changing `prompt.rs` or `layers.rs`. If no narrow change is acceptable, keep that one variant
  inline, say so in SPEC.md, and log it; the ordinary leave question still moves over the dialog.
- If a centered question over a dialog misbehaves in a way the planning review did not foresee (a second Escape closing
  the dialog under it, focus not returning into the dialog), the same rule applies: scope reassessment before touching
  the shared layer or prompt code, and never a fallback to the inline question without logging why.
- If moving the question changes behavior that other tests pin (the Add host footer, Settings' section switching hiding
  the editor's actions), update those tests and SPEC text to the new behavior and log each.

Decisions to log: the name and shape of the "asks on leaving" comparison (derived or stored), how the recipe save's
completion reaches the window, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-templates-dialog-close-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-templates-dialog-close-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/templates-dialog-close/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves or extends the fragment its type calls for, describing in user-facing words
  what was built. Where what got built departs from what this file describes, the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry.

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
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question. The
report says plainly that the cause was found by reading the code and asks the maintainer to confirm the fix on a
devbuild, since the bug cannot be reproduced in the real app unattended.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.

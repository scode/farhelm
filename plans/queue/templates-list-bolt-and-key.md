# Line up the ⚡ and the number key in the Templates list

Written against main at 4638a5334 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Fix the left-hand part of each row in the desktop app's Templates list (the Templates dialog, opened from the session
list's templates button), covering two TODO.md entries under Near term → Trivial, which the maintainer agreed to plan
together because they change the same row code:

- **Vertically center the lightning bolt in the template list.** The maintainer's words: "the layout of the lightning
  bolt in the template menu is off. it needs to be properly vertically centered. (Screenshot: on the Templates page, the
  ⚡ quick-launch marker to the left of each template row sits up near the top of the row, above the ⌘ shortcut, rather
  than centered on the row.)"
- **Something is cut off in the template list's left-hand bar.** The maintainer's words: "wtf is getting cut off in the
  left hand bar. need to understand what it is and whether to keep showing it and if so how, in a way that it's actually
  visible. (Screenshot: on the Templates page, the narrow column left of each template's name holds the ⚡ and a ⌘
  shortcut label, which reads "⌘ –" on rows without a number key and "⌘1" or "⌘0" on rows with one; whatever follows the
  ⌘ looks clipped, and rows showing "⌘ –" indent the template name further right than rows with a number.)"

### Where it stands (from planning, at main 4638a5334)

Line numbers are approximate; paths are under `crates/gui/farhelm-gui/src/`.

- The row is built in `templates/dialog.rs` (about lines 805-990; the left part about 962-989). It is one horizontal
  row, centred vertically, with three parts:
  - The ⚡ column is 28px wide and stretched to the row's full height, with the bolt button at its top. A comment there
    (about line 973, "anchored to the first line") says this was deliberate in the port from the old app.
  - The number-key button is 10px monospace text in a small box, sized to its text and centred on the row.
  - The name and summary take the rest. The ⚡ template's row adds a third line with ⌘P (Ctrl+Shift+P on Linux).
- The ⚡ marks the template that ⌘P launches directly. The label is the template's number key: ⌘ plus a digit
  (Ctrl+Shift plus a digit on Linux) launches it (SPEC.md, the templates and number-key paragraphs, about lines
  1283-1326). "⌘ –" is the placeholder for "no number key" (`templates/shortcuts.rs`, about lines 259-262). Clicking the
  label opens the number-key menu, so it has to stay clickable on rows without a key.
- Nothing in the code clips the label. The "⌘ –" placeholder reads as truncated text. Because the key column is as wide
  as its text, names on "⌘ –" rows start about 6px further right than on "⌘1" rows. On Linux the label reads
  "Ctrl+Shift+ –" or "Ctrl+Shift+2" (about 72px in a 242px row), which squeezes summaries to "Agent launch...".
- Tests pin the placeholder: `templates/shortcuts_tests.rs` (about lines 165-166) and `templates/dialog_tests.rs` (about
  line 843).
- GUI screenshot goldens that picture this row: `templates-dialog`, `template-editor`, `template-leave-question`,
  probably `template-launcher` and possibly `template-number-menu` (`crates/gui/farhelm-gui/tests/goldens.txt`).

### Acceptance criteria

1. The ⚡ and the key cap are vertically centred on the row's name and summary lines. On an ordinary two-line row that
   is the middle of the row. On the ⚡ template's row, the extra ⌘P line hangs below them, so clicking a row's ⚡ (which
   adds that line) does not move the ⚡ or the key cap under the pointer.
2. The number key sits in a key cap of one fixed width that fits the widest label ("⌘0" on the Mac), so every template's
   name starts at the same position whatever its key.
3. A row without a number key shows a dashed, empty key cap holding "+" in place of "⌘ –". Clicking it opens the same
   number-key menu the label opens today; keyboard access to that menu, if the label has it today, is kept.
4. On Linux the cap shows just the digit ("2"); on the Mac it shows "⌘2". The cap's hover help still names the full key
   combination on both.
5. Nothing else about the list changes: the number-key menu's own labels, the ⚡'s behavior, the third ⌘P line, the
   template list opened with ⌘J, and the row's selection and hover look stay as they are.
6. Tests that pin the old placeholder are updated to the new one, a test covers the Linux digit-only label (or the
   function that produces the label, per platform), and the goldens that picture the row are re-captured.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page, where three mockups were shown:

- **One plan for both entries** (the maintainer agreed to the merge).
- **Layout A:** the ⚡ centred; a fixed-width key cap so names line up; a dashed empty cap with "+" for rows without a
  key, which opens the key menu. Rejected: moving the key to the right end of the row, and stacking ⚡ over the key in
  one column.
- **What the ⚡ centres on:** the name and summary lines, not the whole row (follow-up question on the same page, after
  the planning review pointed out that the comment justifying today's top anchoring is about the bolt not moving when
  the ⌘P line appears). Rejected: centring on the whole row, which would drop the ⚡ about half a line on click.
- **Label wording:** just the digit on Linux, "⌘" plus the digit on the Mac. Rejected: digit only on both, and keeping
  the full "Ctrl+Shift+2".
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort with the general review charter.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the two TODO entries quoted above and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md", and
  its "Instructions by path" for every path you touch.
- `.agents/validation.md` for the GUI test selections and the screenshot goldens (re-capturing them with
  `scripts/check-gui-goldens.py --update`, inspecting every changed picture, and publishing them with
  `python3 scripts/gui-goldens.py publish` before any rebase, as it describes).
- `.agents/test-authoring.md` for every test added or changed.
- SPEC.md's templates and number-key paragraphs (about lines 1283-1326). Planning found that neither SPEC.md nor
  SPEC_impl.md names the "⌘ –" label or the column's layout, so no spec change is expected.

**Planner proposals:**

- Change only the row's left part and the label functions (`number_key_name` and the key button's text in
  `templates/shortcuts.rs` and `templates.rs`); keep the fix to one PR.
- Rewrite, rather than delete, the comment at the ⚡ column (about line 973): its reason (the ⚡ must not move when its
  row gains the ⌘P line) still holds and is now met by centring on the first two lines.
- The key cap borrows the look of the Keyboard shortcuts dialog's chip (`shortcuts::chip` in `shortcuts.rs`, about line
  408, which the ⌘J switcher also uses): give the existing borderless key button style (`components/button.rs`, about
  lines 129-132 and 673-676) a fixed width and the chip's border, padding and corner radius, with a dashed border when
  no key is set (dashed borders already appear in `session_list/pin_drag.rs`, about line 752). Matching the values is
  enough; do not build a shared helper for a static chip and a focusable button.

## Outline

1. `plan/templates-list-bolt-and-key/01-bolt-and-key-column`:
   `fix: line up the ⚡ and number key in the templates list`. Centre the ⚡ and key cap on the name and summary lines
   in `templates/dialog.rs`, give the key cap a fixed width, draw the dashed "+" cap for rows without a key, make the
   cap's label the digit alone on Linux, update the placeholder in `templates/shortcuts.rs` and the tests that pin it,
   re-capture and publish the goldens, leave a changelog fragment (`kind: fixed`), and remove both TODO.md entries
   ("Vertically center the lightning bolt in the template list." and "Something is cut off in the template list's
   left-hand bar.").

## Fallbacks

- If one fixed width that fits "⌘0" does not leave the name column enough room in the narrowest supported dialog, size
  the cap for the widest label the platform actually shows and log it as a DECISION.
- If the empty cap cannot carry the key menu's keyboard access the label has today, keep the access and log how.
- Decisions to log: the cap's width and how it was chosen, any golden whose change you judged intended, and every review
  finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-templates-list-bolt-and-key-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-templates-list-bolt-and-key-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/templates-list-bolt-and-key/01-bolt-and-key-column`.
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

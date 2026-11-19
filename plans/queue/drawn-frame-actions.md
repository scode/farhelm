# Actions act on what the user was shown: one standard, one mechanism

Written against main at f2fc9607 on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute four triage outcomes recorded on 2026-11-18, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute
triage outcomes" (one PR per outcome). Their `TRIAGE_OUTCOMES.md` entries are the binding statement of what to do; read
each in full from `main@origin`, together with its feedback file under `review_feedback_queue/`:

1. `header-actions-follow-unseen-session.md` (`fix code`; its Completion criteria carry the standard and the mechanism)
2. `session-actions-use-unseen-liveness.md` (`fix code`, recorded without asking under the clearly-settled rule;
   100-line size gate)
3. `trailing-sandbox-stop-rendered-destructive-guards-refresh-before-dispatch.md` (`fix code`, recorded without asking
   under the clearly-settled rule; 100-line size gate)
4. `wayland-file-drop-uses-entry-position.md` (`fix code`)

All four are one defect class in the desktop app: an action is carried out on state the user was not shown. GPUI
dispatches a mouse event to the handlers of the last frame drawn, which read the newest state when they run
(`vendor/gpui-pre/src/window.rs`, around 6056), so a header button clicked on session A can act on session B after the
selection moved, and a "nothing alive" Delete can carry an "agent alive" precondition after a push. A key event redraws
a dirty window first and dispatches to the new frame's handlers (around 6483-6486), so a keyboard press can act on a
frame that was drawn but never on screen. On Wayland, a file drop is delivered where the drag entered rather than where
it was released.

### The maintainer's instruction, which shapes the whole plan

On the triage page (Q3) the maintainer wrote: "But this must not be done in a random ad hoc way for the two places. We
need to establish a clear pattern that all components can use that have this problem. I don't want this to turn into a
whack-a-mole. We need to set the standard, spec it, and have an implementation that implements the key mechanisms that
get reused whenever this problem arises."

So the plan sets one standard in the specs and builds one reusable mechanism in the desktop app, and every fix in it
goes through that mechanism. Per-site fixes that each capture state their own way are exactly what the maintainer ruled
out.

### What each PR does

1. **The standard and the mechanism; the open session's header and the interrupted card** (`fix:`, with a fragment).
   - **Spec.** Amend SPEC.md's "Confirmed 2026-10-10" paragraph (the sub-second window, in the GUI part of the spec,
     around 5209-5212) rather than adding a separate section, so the spec does not contradict itself: what an action
     targets, and the state its consent or precondition is taken from, are always what was shown when the user acted, by
     pointer or keyboard; a drop lands where it was released. The paragraph's existing allowances (a Cancel and a submit
     arriving together, a dialog hidden behind another) stay. In SPEC_impl.md, extend "Questions: the prompt"
     (around 920) to cover the moment from press to question, and adjust its "No new generic precondition protocol is
     added" sentence so it stays true.
   - **Mechanism.** The existing funnel is `menu::actionable(key, cx)` returning `Actionable { row, skips_questions }`
     (`crates/gui/farhelm-gui/src/session_list/menu.rs`, around 601-626), whose result the header, the card, the row
     menu and the environment Stop and suspend pass to `stop::begin`, `restart::begin`, `replace::begin` and
     `delete::begin`, which already freeze the row into the question they open. Make the mechanism a snapshot of what a
     control showed (the key, the row and the skip flag, plus what the environment verb needs), taken when the control
     is drawn and handed to those same `begin` functions; keep the "refused now" check live, since it can only refuse
     more. A keyboard press must be checked against the frame actually shown, not the one just drawn: the prompt already
     solves this ("Shown, not drawn", `crates/gui/farhelm-gui/src/prompt.rs`, around 20-21, its shown counter and
     `when_shown`, around 980-993 and 1148). Reuse that rather than inventing a second way; moving `when_shown` into a
     shared helper (widening its clippy exemption's stated reason) or keeping it in place and calling it is yours to
     decide, logged as a DECISION.
   - **Users in this PR.** The open session's header controls (Stop, Restart, Replace, Delete, environment
     Suspend/Resume; `session_pane/header.rs`) and the interrupted-session card's Replace (`session_pane/card.rs`).
   - **Tests.** A stale-frame click on the header acts on the drawn session or is refused, never on the newly selected
     one; a keyboard press is checked against the shown frame.
   - The mechanism's size counts against this outcome, which has no size gate.
2. **Lifecycle preconditions come from the shown frame** (`fix:`, with a fragment). Delete, Stop, Restart, Replace and
   the Restart with / Replace with launcher send the precondition matching what the user was shown (SPEC.md "Lifecycle
   operations": "the click authorizes only the state the screen showed"; the launcher "carries the precondition matching
   what the launcher showed at the click"), through PR 1's mechanism: the row menu, skipped confirmations
   (`session_list/delete.rs` and siblings), the launcher (`launcher/view.rs`, around 1215-1221, 1293-1297 and
   1306-1314), and the interrupted card's Restart (`session_pane/card.rs`, around 551-556). Tests: a skipped Delete on a
   stale "nothing alive" frame is refused by the supervisor after the agent restarted. 100-line gate, not counting the
   mechanism.
3. **An environment's Stop and suspend takes its guards and verb from the shown frame** (`fix:`, with a fragment): the
   row menu item and the header button (`session_list/environment.rs`, around 401-466, with `stop.rs`), through the same
   mechanism, with a test. 100-line gate.
4. **A Wayland drop lands where it was released** (`fix:`, with a fragment). In GPUI's Wayland backend
   (`vendor/gpui-pre-linux/src/linux/wayland/client.rs`, around 2924-3011 and 474-507), pointer motion is ignored while
   the drag's file list is being read, and the drag position is then set to the entry point. Record the latest pointer
   position while the read is pending and deliver the drop there. This is a platform coordinate fix, not a use of the
   GUI mechanism; its only tie to the standard is PR 1's spec wording on drops. It is a new patch layer on the vendored
   crate, written as `vendor/README.md`, "GPUI", requires (the tree and its patch file under `vendor/patches/gpui/`
   together, never the tree alone), with a unit test and the vendored GPUI test selection from `.agents/validation.md`.

Each PR removes its feedback file and index line and moves its ledger entry to the archive.

### Acceptance criteria

- SPEC.md's 2026-10-10 paragraph and SPEC_impl.md's "Questions: the prompt" state the standard and the mechanism, and
  neither contradicts the other or any remaining sentence.
- The header, the interrupted card, the row menu, skipped confirmations, the launcher and the environment Stop and
  suspend all act through the one mechanism, for pointer and keyboard; no site captures shown state its own way.
- A Wayland drop is delivered at the release point.
- The tests named above pass through `python3 scripts/record-test-run.py`; each PR records its outcome's execution.
- PRs 2 and 3 stay within their 100-line size gates, the mechanism excluded.

## Decisions already made

- **The outcomes** (2026-11-18 highest-bucket triage page): items 1 and 4 from the maintainer's pick on Q3 ("Yes for the
  target and the drop point"), with the instruction quoted above; items 2 and 3 recorded under the clearly-settled rule
  and amended on 2026-11-18 to use the shared standard and mechanism.
- **Grouping:** the maintainer approved these four as one plan on the 2026-11-18 planning page.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **No release note** (the maintainer, 2026-11-18: every plan from this triage needs none). The `fix:` PRs still leave
  their changelog fragments.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the four `TRIAGE_OUTCOMES.md` entries, the Q3 instruction quoted above, and
the Decisions above.

**Binding repository constraints:** SPEC.md "Lifecycle operations" (confirmed 2026-10-01: a destructive confirmation
authorizes only what the prompt said; the click authorizes only the state the screen showed); SPEC.md's 2026-10-10
paragraph; SPEC_impl.md "Questions: the prompt"; `vendor/README.md`, "GPUI" (patch layers); root `AGENTS.md`, "Tests and
validation", "Changelog fragments", "Conventional Commits", "Code rules"; `.agents/test-authoring.md`;
`review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (from the fresh-context planning review; change them if the code says otherwise, logged as a
DECISION): the mechanism is a shown snapshot of `Actionable` fed to the existing `begin` functions, with the prompt's
shown-frame check reused for keyboard presses; the spec change amends the 2026-10-10 paragraph and "Questions: the
prompt" in place.

## Outline

Four PRs, bottom to top:

1. `plan/drawn-frame-actions/01-shown-state-standard-and-header`
2. `plan/drawn-frame-actions/02-lifecycle-preconditions-from-shown-frame`
3. `plan/drawn-frame-actions/03-environment-stop-from-shown-frame`
4. `plan/drawn-frame-actions/04-wayland-drop-release-point`

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the GUI crates, `python3 -B scripts/check-gui-lints.py` if a lint exemption moves, the GUI's own tests for the touched
modules through the recorder, the GPUI end-to-end smoke if the session list, row menu, header, Delete, Stop, Restart or
Replace behavior changes visibly, the vendored gpui-pre test selection for PR 4, and `dprint check` on the spec files.

## Fallbacks

- If PR 2 or PR 3 would exceed its 100-line gate even with the mechanism counted against PR 1, block and bring it back,
  as the ledger says.
- The marked-sessions bar's Stop and Delete (`session_list/bulk.rs`, around 10-14) take their targets from the drawn
  frame but their guards from the store at the press. No ledger entry covers them: do not change them; name them in the
  report's "open questions and possible follow-ups".
- Examples of departures that need the scope reassessment: a second capture mechanism beside the prompt's, a generic
  precondition protocol on the helm or supervisor, routing the Wayland drop through the GUI mechanism, or changing what
  the supervisor's guards check.

Decisions to log: the mechanism's shape and where the shown-frame check lives, and every review finding you decided not
to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-drawn-frame-actions-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-drawn-frame-actions-watchdog.json` beside it. Resolve both to
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

- One reviewable commit, stable bookmark and draft PR per outcome, in a single linear stack, using the `jjstack` skill,
  as `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes", requires (the outline above names
  any agreed exception). The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/drawn-frame-actions/<nn>-<short-name>`, as the outline names them.
- Each PR carries its outcome's agreed change, removes the outcome's feedback file and its
  `review_feedback_queue/INDEX.md` line, and moves the outcome's `TRIAGE_OUTCOMES.md` entry, with its final Execution
  field, to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (`review_feedback_queue/AGENTS.md`, "Lifecycle"). Record the change
  ID and bookmark in the Execution field before creating the PR, then add the PR's URL to the same change and update the
  PR; never make a separate bookkeeping PR.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"), whatever this plan's Release note section says.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

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
already made and outline, and the outcome's `TRIAGE_OUTCOMES.md` entry). When a PR changes tests or their fixtures,
include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
working log with the true value; never edit this plan file. If current code or specs invalidate an outcome's decision
(`review_feedback_queue/.agents/execute-outcomes.md`), that outcome is a block, not something to reinterpret.

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
meeting the acceptance criteria above, each having passed its review gate, and each having recorded its outcome's
execution and moved its ledger entry to the archive. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

## Decisions

### 2026-11-19: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-19 (claim 80124b).
>
> ### Session actions and the launcher's follow-up warning
>
> The desktop can still show an ended agent while a newer update says it is running. If the user chooses Delete from
> that row with confirmations skipped, the new agent can be stopped. The agreed fix makes the session menu,
> interrupted-session card and Restart with or Replace with launcher use the state the user saw. This part of the fix
> has a 100-line limit for code, tests and docs, excluding the shared frame mechanism and bookkeeping.
>
> The launcher can ask separately for permission to start a YOLO session. For example, the first press of Replace can
> warn that it will stop the old agent. If that agent exits while the follow-up is open, the warning can disappear.
> Answering the follow-up at that point can nevertheless carry the old permission to delete a running agent, and kill
> one that restarts before the delete runs. The answer can also wait for a host setting to save or a checkout preview to
> arrive. The fix needs to use the warning the user saw at the answer through either wait.
>
> The smallest attempted correction that accounts for those waits and Cancel changed 129 lines under the same counting
> rule. Reusing an existing supervisor test would reduce it to 118. That candidate also froze the warning during the
> follow-up. I recommend keeping the warning live as specified and binding each answer to the warning it displayed.
>
> Should the remaining lifecycle work be split into two reviewable PRs, or have a higher limit for one PR? I recommend
> splitting the session menu and interrupted card from the launcher. The draft stack would contain both before the
> monitor lands the plan. Review could check the smaller GUI changes separately from the launcher timing. A single
> larger PR would put the complete fix in one landing commit and needs an exception to the 100-line limit.
>
> Three independent fixes are in draft PRs. The session header and interrupted card’s Replace are in
> [#4176](https://github.com/scode/farhelm/pull/4176/changes), Stop and suspend is in
> [#4181](https://github.com/scode/farhelm/pull/4181/changes), and the Wayland drop correction is in
> [#4183](https://github.com/scode/farhelm/pull/4183/changes).

The maintainer's answer:

### Answered by the landing monitor on the maintainer's behalf

The landing monitor answered this block on the maintainer's behalf, under the maintainer's standing rule that only a
huge question waits for them (`plans/.agents/landing.md`, "Answering blocks"). A later change of mind is a follow-up,
not a reason to stop the plan.

- **Split the remaining lifecycle work into two PRs**, as recommended: one for the session menu and the interrupted
  session card, one for the Restart with and Replace with launcher's follow-up question. Each is reviewed on its own,
  and both sit in the plan's draft stack before it lands.
- **Keep the warning live, as specified, and bind each answer to the warning it displayed.** Do not freeze the warning
  while the follow-up question is open. An answer given after the warning changed, including one that waited for a host
  setting to save or a checkout preview to arrive, acts on the warning the user saw when they answered, never on an
  earlier one; Cancel behaves the same way.
- **The 100-line limit applies to each of the two PRs.** If the launcher PR needs more to cover both waits and Cancel
  correctly (the attempt that did was 129 lines, 118 reusing an existing supervisor test), it may go up to about 140
  lines under the same counting rule; say in the report what it came to. Reuse the existing supervisor test where it
  fits. If it would need much more than that, block again rather than cutting coverage of either wait.

### 2026-11-19: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-19 (claim cf10b9).
>
> ### The launcher's warning and its delayed answers
>
> Restart with and Replace with can ask separately for permission to start a YOLO session. The warning about the
> original session can change while that question is open. If the original agent exits, the warning can disappear. It
> can then restart while an answer waits for the host setting to save or a checkout preview to finish. Replace can then
> stop the restarted agent even though no warning was visible when the user answered.
>
> The earlier decision split the menu and interrupted card from the launcher, kept the warning live, and allowed about
> 140 changed lines for the launcher with both waits and Cancel covered. The menu and card are within their limit. The
> launcher trial is 231 added or removed lines after excluding the shared presentation helper. It carries the warning's
> permission through the two existing waits and adds tests that change the original session during each wait. A separate
> scope review found no credible way to fit both waits and their tests within about 140 lines. The checkout-preview and
> Cancel cases passed. The host-setting test stopped before it could check the retry.
>
> Can the remaining launcher work use a roughly 240-line budget in one PR? I recommend allowing it. The main button and
> each follow-up answer would share one definition of what its warning authorizes. A second option is to split the
> launcher's main-button fix from its follow-up fix, with a separate size allowance for the follow-up. That keeps each
> review focused but leaves a draft boundary where the initial button is fixed and the later answer still needs work.
> Either option keeps the warning live and covers both waits and Cancel.
>
> The draft stack contains the session header and interrupted card’s Replace
> [#4176](https://github.com/scode/farhelm/pull/4176/changes), the session menu and interrupted card’s Restart
> [#4193](https://github.com/scode/farhelm/pull/4193/changes), environment Stop and suspend
> [#4181](https://github.com/scode/farhelm/pull/4181/changes), and the Wayland drop correction
> [#4183](https://github.com/scode/farhelm/pull/4183/changes). The launcher trial is retained for the next executor to
> finish and review.

The maintainer's answer:

### Answered by the landing monitor on the maintainer's behalf

The landing monitor answered this block on the maintainer's behalf, under the maintainer's standing rule that only a
huge question waits for them (`plans/.agents/landing.md`, "Answering blocks"). A later change of mind is a follow-up,
not a reason to stop the plan.

- **The remaining launcher work may use about 240 changed lines in one PR**, under the same counting rule, as
  recommended. The main button and each follow-up answer share one definition of what the warning they showed
  authorizes, which a split would separate. The warning stays live, and both waits (the host setting's save and the
  checkout preview) and Cancel stay covered by tests.
- Finish the host-setting test so it checks the retry, and say in the report what the PR came to. If it would need much
  more than about 240 lines, block again rather than dropping coverage of either wait.

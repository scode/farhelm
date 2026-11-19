# Fifteen supervisor and terminal-host test gaps from the 2026-11-18 GUI UX triage

Written against main at e3068e2bd on 2026-11-18. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Execute fifteen `fix code` outcomes recorded on 2026-11-18, each a test or fixture that could pass without proving what
it claims, flake, hang past its deadline or leave a helper behind, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes" (one PR per outcome). Production behaves
correctly in every case. Their `TRIAGE_OUTCOMES.md` entries are the binding statement of what to do; read each in full
from `main@origin`, together with its feedback file under `review_feedback_queue/`:

1. `stale-reply-test-lacks-processing-witness.md` (`fix code`): Late-reply test can't tell matched reply from newest
   reply. fix: send reply 1, assert the second call still waits, then reply 2 (~10-15 lines).
2. `process-presence-fixture-leaves-sleep-descendant.md` (`fix code`): Passing test leaves a sleep running up to 30 s.
   fix if a group kill or childless stand-in fits (~5-10), else discard.
3. `checkpoint-rejection-test-asserts-before-processing.md` (`fix code`): Rejected-checkpoint check runs before the
   rejection is read. fix: move the assertion after the status frame (~3 lines).
4. `tab-reap-deadline-excludes-listing-request.md` (`fix code`): Polling helper's 15 s limit doesn't cover a stuck
   check. fix: wrap each check in the deadline (~5-8).
5. `keeper-can-win-one-shot-interrupted-start-gate.md` (`fix code`): Second session can take the gate meant for the
   first. fix: copy the sibling test's held marker (~10-15).
6. `stty-tool-format-mismatch.md` (`fix code`): Terminal-settings test compares two different stty programs. fix: give
   the reference command the host's PATH (~2 lines).
7. `notification-cleanup-row-premise.md` (`fix code`): Upgrade test checks notification cleanup without first showing it
   exists. fix: assert the count is 1 before the delete (~6).
8. `successor-exit-processing-witness.md` (`fix code`): Exit-code test may read before the new supervisor caught up.
   fix: wait for the dead agent first (~3).
9. `tab-survival-missing-live-exchange.md` (`fix code`): Tab-survival test never asks the shell to do anything after
   restart. fix: run a command in the shell after reattach (~8-10, existing helper).
10. `inherited-lock-test-self-dup.md` (`fix code`): Version-lock test may never hand the lock over. fix: copy the
    sibling's guard and assert the premise (~10-15).
11. `stderr-readiness-lacks-live-holder-premise.md` (`fix code`): "Don't wait for stderr" test never checks something
    holds stderr. fix: require the holder pid and assert it alive (~5-10).
12. `late-listener-blocks-start-test-cleanup.md` (`fix code`): A slow stand-in listener can hang a test until the runner
    kills it. fix: accept with a deadline (~10-15).
13. `readiness-test-does-not-establish-first-stage-exit.md` (`fix code`): "Start returns after its first stage exits"
    test can't tell. fix: also assert the parent isn't a live child (~5-10).
14. `recording-loader-rejects-valid-json-field-spacing.md` (`fix code`): Private-recording loader rejects JSON with a
    space before the colon. fix: skip whitespace before the colon (~5).
15. `trailing-converted-old-session-warning-retention-assertion-vacuous-all.md` (`fix code`): Old-terminal test passes
    even if the old warning vanished. fix: assert exactly one resolved warning (~3-5).

### What each PR does

Each entry's completion criteria name the fix and carry the maintainer's rule: fix the test only if the fix is obvious
and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat the outcome as a discard
and say so in the execution record. Either way the PR removes the feedback file and index line and moves the ledger
entry to the archive. Test-only PRs are `test:` with no changelog fragment; a PR that turns into a discard is `chore:`.
Read `.agents/test-authoring.md` before changing any test, and run `python -B scripts/check-test-sleeps.py` as root
`AGENTS.md` says for Rust test changes.

Each PR removes its feedback file and index line and moves its ledger entry to the archive.

### Acceptance criteria

- Fifteen PRs, each meeting its ledger entry's completion criteria (fixed within the ~30-line rule, or recorded as a
  discard with the reason) and recording its execution.
- Each fixed test still passes, and where the entry names the regression it should catch, the test now fails against
  that regression (checked by a temporary local mutation, not committed).

## Decisions already made

- **The outcomes:** the maintainer's picks on the 2026-11-18 GUI UX triage page (questions QA to QG, rounds 1 to 5), or
  the clearly-settled rule where an entry says so.
- **Grouping:** the maintainer approved these as one plan on the 2026-11-18 planning page (Q1, seven plans for the day's
  111 outcomes).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (the maintainer's pick, planning page Q3; see
  Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes (planning page Q4).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **No release note** (the maintainer, 2026-11-18: all of these are "no release note needed"). PRs whose type calls for
  one still leave their changelog fragments.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the `TRIAGE_OUTCOMES.md` entries listed above and the Decisions above.

**Binding repository constraints:** `.agents/test-authoring.md`, the test-sleep check,
`review_feedback_queue/.agents/execute-outcomes.md`; root `AGENTS.md`, "Tests and validation", "Conventional Commits";
`crates/terminal-host/SPEC.md`, "Change policy", for anything under `crates/terminal-host/` (these are test-only
changes).

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): each fix follows the shape its
ledger entry names; where an existing helper or sibling test already does it, reuse it.

## Outline

15 PRs, bottom to top, in the order of the list above:

1. `plan/gui-ux-backend-test-gaps/01-stale-reply-test-lacks`
2. `plan/gui-ux-backend-test-gaps/02-process-presence-fixture-leaves`
3. `plan/gui-ux-backend-test-gaps/03-checkpoint-rejection-test-asserts`
4. `plan/gui-ux-backend-test-gaps/04-tab-reap-deadline-excludes`
5. `plan/gui-ux-backend-test-gaps/05-keeper-can-win-one`
6. `plan/gui-ux-backend-test-gaps/06-stty-tool-format-mismatch`
7. `plan/gui-ux-backend-test-gaps/07-notification-cleanup-row-premise`
8. `plan/gui-ux-backend-test-gaps/08-successor-exit-processing-witness`
9. `plan/gui-ux-backend-test-gaps/09-tab-survival-missing-live`
10. `plan/gui-ux-backend-test-gaps/10-inherited-lock-test-self`
11. `plan/gui-ux-backend-test-gaps/11-stderr-readiness-lacks-live`
12. `plan/gui-ux-backend-test-gaps/12-late-listener-blocks-start`
13. `plan/gui-ux-backend-test-gaps/13-readiness-test-does-not`
14. `plan/gui-ux-backend-test-gaps/14-recording-loader-rejects-valid`
15. `plan/gui-ux-backend-test-gaps/15-converted-old-session-warning`

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: run each changed test, narrowly, through
`python3 scripts/record-test-run.py` (`.agents/narrow-tests.md` has the selections), plus `cargo fmt` check, clippy on
the touched crates and the test-sleep check. Report checks run, reused and skipped.

## Fallbacks

- A fix that needs more than about 30 lines, a new test mechanism or more than one plausible shape becomes a discard at
  execution, as its entry says; that is not a block.
- A test the entry names that no longer exists on main: record the outcome as a discard ("test gone") and say so.

Decisions to log: each outcome turned into a discard and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-gui-ux-backend-test-gaps-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-gui-ux-backend-test-gaps-watchdog.json` beside it. Resolve
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

- One reviewable commit, stable bookmark and draft PR per outcome, in a single linear stack, using the `jjstack` skill,
  as `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes", requires (the outline above names
  any agreed exception). The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/gui-ux-backend-test-gaps/<nn>-<short-name>`, as the outline names them.
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

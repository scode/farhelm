# UI: refuse Cancel while Replace's YOLO "don't ask again" is saving

Written against main at aa245430 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Cancel racing "don't ask again" on Replace's YOLO question.** Clicking Cancel in the same instant as "don't ask
> again" on a session's Replace YOLO question hides the question, but the Replace still proceeds with YOLO, and a failed
> save of that setting is shown nowhere. Complexity gate: fix it only if the fix is simple; otherwise leave it. From the
> review of `bind-gui-answers` on 2026-10-23.

The race, from a read of main at aa245430 (line numbers approximate). "The same instant" means both clicks are
dispatched before the render that draws the buttons disabled:

1. "Don't ask again" on the session surface's question (`crates/farhelm-ui/src/session_view.rs` around 2447-2460) calls
   `ReplaceController::answer_session(&question, true)` (`crates/farhelm-ui/src/replace.rs` around 334-357). A permanent
   answer deliberately leaves the question open (the session policy keeps it mounted through the save), then the
   `replace` closure sets `replacing` synchronously (`session_view.rs` around 1390) and spawns `run`, which saves the
   host setting first and then sends the Replace with `allow_yolo` captured before any await (`replace.rs` around
   441-477).
2. Cancel (`session_view.rs` around 2461) calls `cancel_question` (`replace.rs` around 361-366) with no busy or claim
   check. The token is still current, so the question closes, its error is cleared and its retry key ended.
3. The task carries on. A successful save replaces with YOLO, which the user cancelled. A failed save stores its reason
   on a question no longer shown, and the surface ignores the `QuestionHandled` result (around 1406).

Only Cancel can follow in the burst: a second answer fails because the running task holds the operation claim. The
sidebar's copy of the question is not affected, because it consumes the question before the save. The code already
documents the gap: `crates/farhelm-ui/src/ops.rs` (around 385-392, `ConfirmSlot` docs) and SPEC_impl.md (around 798-803,
"Answers bind to what was drawn": the first three stay-up prompts refuse a second answer while their request runs, "the
YOLO question does not yet"). It breaks SPEC.md (around 905-908): if changing that setting fails, nothing is started and
the confirmation stays up with the reason.

### Acceptance criteria

- While the session surface's Replace is running (the window in which its question's buttons are drawn disabled), the
  question's Cancel does nothing and shows no notice. The question stays up, the source exclusion and retry key stay as
  they were, and the outcome follows the first answer: a successful save proceeds with the Replace, and a failed save
  shows its reason on the question through the existing error line, with nothing started.
- Outside that window Cancel behaves as today. The sidebar's question is unchanged.
- The refusal keys on the surface's own `replacing` state, not on the page-wide operation claim, which unrelated
  operations also hold and which would refuse Cancel on an idle question.
- Docs updated to the new contract: the `ConfirmSlot` docs in `ops.rs`, SPEC_impl.md's "Answers bind to what was drawn"
  paragraph (all four stay-up prompts now refuse a second answer while their request runs), and the `replace.rs` docs on
  `session()`, `answer_session` and `cancel_question`. SPEC.md needs no change: the fix brings the code in line with it.
- Tests:
  - A browser test that pins the session surface's wiring, which the unit test cannot see: with the host-setting save
    refused (`injectYoloReplaceSession` with `control.refuseMark` in `e2e/tests/terminal-restart.spec.ts`, around
    395-517), click "don't ask again" and then Cancel in one `evaluate`, asserting Cancel was still live when clicked,
    as `e2e/tests/yolo-guard.spec.ts` (around 340) does for the launcher's question. Expect the question still visible
    with its error line, the mark attempted, and no Replace with the override sent. Run it on both browser engines.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Refuse Cancel while the save and the Replace run**, chosen over "Cancel wins" (aborting the Replace). The host
  setting write cannot be undone, so Cancel winning would leave the host marked "don't ask again" with no Replace done;
  once the save finishes the Replace is already sent; and a failed save would need a new place to show. Refusing matches
  the disabled buttons and the other three prompts that stay up through their request.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

Stated to the maintainer on the planning page, without objection: the fix passes the entry's complexity gate, and the
browser test may share a known flake (below).

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's YOLO confirmation rules (around 905-908) and its rule that failed
operations surface an error (around 2845); SPEC_impl.md's "Answers bind to what was drawn" paragraph; root `AGENTS.md`,
"Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- A one-line guard in the session view's Cancel handler (`if *replacing.peek() { return; }`, `session_view.rs` around
  2461), with a comment saying why. `replacing` is set synchronously before the spawn, so a Cancel dispatched in the
  same burst reads it as true. The browser test pins this wiring, which is what can regress; the existing `replace.rs`
  tests (`permanent_answer_policy_preserves_the_host_save_boundary`, `session_answers_only_the_rendered_opening`)
  already cover a failed save leaving its reason on an open session question. Not chosen: a `busy` parameter on
  `cancel_question`, which would only add a constant `false` at the sidebar and four existing test call sites.
- The known flake: the existing test of this flow in `terminal-restart.spec.ts` is an open flake (`FLAKES.md`,
  "interrupted-card Replace clicks lost under load"; `deflake/known-flakes.txt`). If a path to the session surface's
  question other than the interrupted card exists (the header's Replace, say), prefer it for the new test. If the new
  test can only go through the card, keep it, and a failure that matches that flake's signature is the known flake, not
  a regression; follow `.agents/test-evidence.md` for it.

## Outline

One PR, `plan/replace-yolo-cancel-race/01-refuse-cancel`: the guard, the doc updates, the browser test, the changelog
fragment and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-ui`, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares, and
through `python3 scripts/record-test-run.py` the new browser test and the existing Replace YOLO tests in
`terminal-restart.spec.ts` and `yolo-guard.spec.ts` on both engines.

## Fallbacks

Examples of departures that need the scope reassessment: Cancel aborting the Replace, reverting the host setting, a new
place to report the failure, or changing the shared `YoloConfirmation` component's behavior for the launcher or sidebar.

Decisions to log: the refusal's final shape, the path the browser test takes to the question, every test changed and
why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-replace-yolo-cancel-race-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/replace-yolo-cancel-race/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.

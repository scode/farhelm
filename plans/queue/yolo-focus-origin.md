# Questions after a click take focus only from where the click left it

Written against main at cb501937 on 2026-11-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Plan decisions to revisit"), verbatim:

> **Revisit letting a plain Replace's YOLO question take focus while the user has pressed nothing since.** Decided by
> the landing monitor on 2026-11-10 while landing nonblocking-session-actions, under the maintainer's instruction to fix
> that landing's problems itself. The plan's rule was that a question arriving after a reply takes focus only when
> nothing else holds it. In WebKit, the desktop app's engine family, a header button click does not take focus, so after
> a header Replace refused as a YOLO launch focus was still in the session's terminal and Cancel was left unfocused (7
> of 10 runs). Now the question focuses Cancel outright while the page's key/pointer press count still equals the count
> at the user's Replace click, and keeps the guard otherwise. The alternative was to record which element held focus at
> the click and let the question take focus only from that element, which needs per-request focus bookkeeping.

The maintainer chose the alternative, sharpened: a question that opens after an asynchronous reply to the user's own
click takes focus only if nothing has been pressed since the click **and** focus is still where the click left it. It
applies through one shared helper to Replace's YOLO question and to a sandbox session's two lifecycle questions.

### Where things are (as of cb501937; correct line numbers to the truth as you go)

- Press count: `crates/farhelm-ui/assets/launch-activity.js`, `installLaunchActivity` (around 16-101), one capture-phase
  `keydown`/`pointerdown` listener on `window` keeping `window.__farhelmLaunchPressCount`, ignoring auto-repeat, a
  modifier alone, and a repeated press on the launcher's own submit (`repeatsSubmit`). Rust side:
  `crates/farhelm-ui/src/launch_activity.rs` (`LaunchActivity::snapshot` reads the count synchronously inside a
  handler).
- Guard: `FocusTarget::async_script` / `async_focus_script` in `crates/farhelm-ui/src/modal_dialog.rs` (around 365-400):
  focus moves only if the active element is the body, the document element, or inside the surface being left. Check and
  move run in one script.
- Today's exception: `crates/farhelm-ui/src/yolo_confirm.rs`, `focus_cancel` (around 221-253): while the press count
  equals the baseline read at the user's Replace answer, focus Cancel outright; otherwise the guard. The baseline flows
  through `replace.rs` (`PendingReplace::press_baseline`, `ReplaceAttempt::with_press_baseline`) from `do_replace` in
  `crates/farhelm-ui/src/list/view.rs` (read with `launch_activity.snapshot()` inside the click). Header Replace:
  `session_view.rs` (button, `on_replace`, the anchored confirmation around 2559-2611); row Replace goes through the
  same `do_replace`.
- Sandbox questions: `crates/farhelm-ui/src/sandbox_sessions.rs`, `Controls`: the Stop-and-suspend question mounts after
  an asynchronous activity read and focuses through `async_script(None)` (around 254-263), which in WebKit leaves it
  unfocused after a header click; the plain Suspend question (around 265) focuses unguarded through
  `focus_confirmation`, whatever the user did meanwhile.
- Spec text: SPEC_impl.md "GUI: Dioxus" around 1160-1166 and 1177-1179 (the exception), SPEC.md around 1392-1394 (the
  user-facing statement).

### Acceptance criteria

1. One shared helper decides focus for a question opened after an asynchronous reply to the user's click: it focuses the
   question's target only if the press count still equals the count read at that click and the focused element is the
   page body (or document element), the element that held focus when the click's press began, or the button the click
   pressed (see Decisions for focus the app itself placed while handling that click). Otherwise it falls back to today's
   guard. An unknown count keeps the guard.
2. It is used by Replace's YOLO question (row and header, replacing the count-only exception), a sandbox session's
   Stop-and-suspend question, and its plain Suspend question.
3. On both Chromium and WebKit: each of those questions takes focus after the user's click on every path that reaches it
   (header and row, with the confirmation shown and skipped where there is one, row menu and header for the sandbox
   questions); and it leaves focus alone when, between the click and the question, focus moved without a press (moved
   programmatically in the test) or the user pressed something.
4. SPEC.md and SPEC_impl.md state the new rule and its users in the places above.
5. The PR removes the TODO entry.

## Decisions already made

Settled by the maintainer on the 2026-11-11 planning page:

- **The rule:** take focus only if nothing was pressed since and focus is still on the page body, the element focused at
  the click, or the button clicked. Rejected: keeping today's count-only exception.
- **Scope:** Replace's YOLO question plus a sandbox session's Stop-and-suspend and Suspend questions, through one shared
  helper. Not the Hosts panel's Suspend question; every other asynchronous focus move stays with the Near term entry
  "Stop asynchronous completions from moving focus or closing things the user has moved on from, across the whole UI".
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Size gate:** about 300 lines of diff (code, tests and docs, not counting the changelog fragment or the TODO.md
  removal). Over it, block and bring it back.

Planner interpretation, written after the planning review found it (the maintainer sees it when approving this plan):
some paths move focus as part of handling the click itself before the reply arrives. A row menu's teardown hands focus
back to the row's `⋯` toggle (`list/row.rs` around 1000 and 1449), and header Replace's anchored confirmation focuses
its own danger button (`session_view.rs` around 2578-2596). Read literally, the rule would deny the question focus on
those paths, a regression from today (plain Suspend focuses unguarded, Replace by count only). The rule's intent is to
protect focus the user moved elsewhere, so **focus that the app's own synchronous handling of the click placed counts as
"where the click left it"**: record it once the click's handling settles, alongside the element focused at the press and
the pressed button. If a path cannot be handled that way without trusting focus a later, unrelated event placed, block
with the measured paths and the options.

## Requirement sources

**The user's request:** the entry above and the decisions just listed.

**Binding repository constraints:** SPEC.md and SPEC_impl.md as cited; root `AGENTS.md`, "Tests and validation",
"Changelog fragments", "Conventional Commits", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Keep only the latest counted press: one record `{count, focused, button}` (plus the settled post-handling focus),
  overwritten on each counted press and left alone by the presses the counter ignores. Because the helper requires the
  count to equal the baseline, the latest counted press is always the user's click. No history, no per-request keys.
  WeakRefs are optional; a strong reference to at most a couple of detached nodes is fine. No feature checks.
- The helper sits beside `async_focus_script` in `modal_dialog.rs`, taking the baseline; `focus_cancel` and the sandbox
  questions call it with a baseline taken by `launch_activity.snapshot()` inside the click (the sandbox start callback
  runs synchronously in the click handler).
- Before writing the rule, measure on both engines which element holds focus when each reply lands, on every path in
  acceptance criterion 3, and log it; the tests then pin those paths.

## Outline

One PR: `plan/yolo-focus-origin/01-click-origin-focus`. `fix:` with a changelog fragment. Browser tests in
`e2e/tests/session-action-completion.spec.ts` (its Replace tests around 51-131 are the model) and the sandbox session
spec that covers Stop and suspend (`e2e/tests/tensorlake-sandbox-sessions.spec.ts`), with repeated WebKit runs: the
original bug showed in 7 of 10.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-ui`, the touched unit tests, the touched browser specs on Chromium and WebKit (WebKit repeated) through
`python3 scripts/record-test-run.py`, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and
`python3 releasing/check-changelog.py format`. No cloud calls.

## Fallbacks

Examples of departures that need the scope reassessment: applying the helper to surfaces beyond the three questions,
changing the press counter's semantics for its other users (New/Clone success, the quick-launch naming wait), or
per-request focus bookkeeping.

Decisions to log: the measured focus owner per path and engine, how post-handling focus is recorded, the helper's shape,
and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-yolo-focus-origin-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/yolo-focus-origin/<nn>-<short-name>`, as
  the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
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
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

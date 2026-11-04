# Launches: show that a slow launch is still in flight

Written against main at 5e155da1 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Better feedback when launching sessions.** When a remote host is slow, a launch can feel hung: it is not clear
> whether the session is succeeding, failing, still in flight, or stuck. Make the launch's state and progress visible so
> a slow host reads as in flight rather than dead.

NOTE: This is a GUI-only change. It shows a live "still working" status with elapsed time while a launch, Restart or
Replace is waiting on the helm, and stops claiming "Not launched." when the GUI merely gave up waiting. It adds no helm
state, no protocol message and no supervisor change, and it does not make launches faster or cancellable.

### Acceptance criteria

- **The session launcher** (New, Clone, Replace with, Restart with): once a launch request has been in flight for about
  2 seconds, the launcher's footer shows, beside the greyed buttons, a spinner and a live status naming the host and the
  elapsed seconds, for example "Launching on buildbox… 14 s" (Restart with: "Restarting on buildbox… 14 s"; Replace with
  in the same pattern), updated about once a second. It disappears when the reply arrives. Nothing changes for a launch
  that answers within the threshold.
- **Hidden launches** (quick launch and template shortcuts, which run the launcher hidden): after the same threshold, a
  small non-blocking pill at the bottom of the window shows the same status, naming what is being launched where it has
  a name (for example "Launching “fix-flaky-test” on buildbox… 9 s"). It does not block the rest of the window beyond
  what the in-flight launch already blocks today, and it goes away when the reply arrives.
- **When the GUI's 60 s request deadline expires** on a launch whose outcome is therefore unknown, the alert no longer
  starts "Not launched.". It says the outcome is unknown, along the lines of the schematic the maintainer approved:
  "Still waiting on buildbox. The session may still start; it will appear in the sidebar if it does. Launching again
  with the same settings will not start a second copy." The launcher stays open, as today. A launch the helm actually
  refused, and every other failure that is not the deadline expiring, keeps today's alert unchanged. The deadline expiry
  is recognized by a dedicated signal, not by today's "unresolved" outcome (see What the code looks like).
- **Restart and Replace** (plain Restart from the session, and plain Replace from every surface SPEC.md says behaves
  identically: the sidebar, the session header and the interrupted-session card) get the same in-flight status beside
  their buttons after the same threshold, and outcome-unknown wording when their deadline expires. A restart sends no
  retry key, so its wording must not promise that trying again is harmless; say instead that it may still finish and to
  check the session before restarting again (wording yours, logged). Replace's wording promises only what its retry
  behavior actually guarantees.
- **SPEC.md, "Creation"** (around 1051-1062) describes the in-flight status and the outcome-unknown wording, and the
  matching Restart / Replace passages say the same; SPEC_impl.md gets whatever implementation note the mechanism needs.
- **Docs**: the docs page that describes launching (find it under `website/src/content/docs/docs/`) mentions the status
  if it describes what the user sees while launching; otherwise leave the docs alone and say so in the report.
- **Tests** prove the status appears after the threshold and not before, counts up, names the host, disappears on the
  reply, appears as the pill for a quick launch, and that a deadline expiry shows the outcome-unknown wording while a
  real refusal still shows "Not launched.".
- **Changelog and TODO:** a changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes
  the TODO.md entry, including its `Plan:` reference.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page (each was the recommended choice):

1. **Where:** in the launcher's footer, plus a pill for hidden launches (quick launch, template shortcuts). Held by the
   GUI only, with no helm or protocol change. Not chosen: a "Launching…" placeholder row in the sidebar, either held by
   the starting window or by the helm (the helm-held row fits better after the separate TODO entry "Treat the GUI–helm
   connection as infallible").
2. **Elapsed time only**, no named steps. Named steps would need new supervisor messages and a protocol bump that forces
   every remote host to update, and a fresh checkout's clone already shows its progress in the session's terminal.
3. **Deadline expiry:** say the outcome is unknown, as in the approved schematic, and keep the launcher open. Not
   chosen: also auto-selecting the session when it appears, or removing the 60 s deadline for launches.
4. **Restart and Replace from the session's own buttons are included**, with the same status and the same expiry
   wording.
5. **No cancel button** (stated on the page, not contested): the helm owns the launch and the host cannot abort it
   midway; the user can delete the session once it appears.
6. **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
   be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute
   reviewer.
7. **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above. The three schematics the maintainer approved were:
the launcher footer "⟳ Launching on **buildbox**… 14 s" beside greyed Cancel and Launch; the pill "⟳ Launching
“fix-flaky-test” on **buildbox**… 9 s"; and the expiry alert "⚠ **Still waiting on buildbox.** The session may still
start; it will appear in the sidebar if it does. Launching again with the same settings will not start a second copy."
Treat those as the approved wording and layout; adapt only what the real components force (log a DECISION).

**Binding repository constraints:**

- SPEC.md, "Creation" (around 1036-1062): the precondition/launch failure split, the "Not launched." / "Restart error."
  alert, and the lost-reply retry that "may already have started the session" (around 1040).
- SPEC.md, "Slow hosts" (around 3614-3626): "Do not add complexity to compensate for a slow remote host." This plan
  shows the wait; it must not engineer around it (no retries, no prefetching, no faster paths).
- SPEC.md around 3922-3926: in the desktop app an action "either completes and reports its outcome, or reports that its
  outcome is unknown". Today's "Not launched." on a deadline expiry breaks that rule; the new wording satisfies it.
- SPEC.md around 1005-1019 and 1236-1243, and SPEC_impl.md around 183-197: a lost reply keeps its retry key and is
  reconciled, so launching again with the same settings reaches the same session. The new launcher wording promises
  exactly that; verify it holds for New, Clone, Replace with and quick launch. Restart and Restart with send no retry
  key (`restart_session`), so their wording makes no such promise. A fresh `gh:` checkout's error text already says the
  request is kept and a retry reconciles it; do not repeat that in two places.
- SPEC.md around 1013-1016: plain Replace from the sidebar, the session header and the interrupted-session card behave
  identically.
- Root `AGENTS.md`: "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation",
  "Talking to the user", and "Instructions by path" (`website/AGENTS.md` before a docs edit).
- `.agents/test-authoring.md` for the test changes, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`. The
  threshold and the counter are time-based; tests drive them without real sleeps (a controllable clock, or held routes
  plus the page's own timers driven deterministically), as `.agents/test-authoring.md` and the sleep checker require.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Two PRs: `plan/launch-progress/01-launcher-status` (the launcher footer, the hidden-launch pill, the expiry wording
  for every launcher mode, SPEC.md "Creation", the fragment) and `plan/launch-progress/02-restart-replace-status` (plain
  Restart and Replace from the session's buttons, their SPEC passages, and the TODO.md removal). One PR is fine if the
  shared piece makes the split artificial.
- One small shared status component (spinner, text, elapsed counter starting after the threshold) used by the launcher
  footer, the pill and the session buttons, rather than three copies. The 2 s threshold matches the `farhelm` command
  line's existing "waiting for the helm…" notice after 2 s (`crates/farhelm/src/agent_client.rs` around 439-443).
- Reuse the delete spinner's styles (`.delete-progress` and `.delete-spinner` in `crates/farhelm-ui/assets/app.css`,
  around 5349) for the spinner.
- The pill for a hidden launch renders inside that launch's existing transparent backdrop, which already blocks what an
  in-flight launch blocks, so it needs no new layer.
- Put the plain-Replace status and expiry wording in the shared Replace controller (`crates/farhelm-ui/src/replace.rs`)
  so all three Replace surfaces get it at once.
- The status is announced politely to assistive technology once when it appears, not on every second's tick.

## What the code looks like at 5e155da1

Line numbers are approximate; verify before relying on them.

**The launcher** is `crates/farhelm-ui/src/list/create_form.rs`, one component for New, Clone, Replace with, Restart
with and quick launch. Submit runs from around 4950; the network call forks around 5724-5827: a fresh `gh:` checkout
goes to `api::submit_fresh_create`, Replace with to `api::replace_session_with`, an ordinary launch to `create_session`
(`crates/farhelm-ui/src/api.rs` around 2572), Restart with to `restart_session` (`api.rs` around 3237, called around
5321). While the request runs, the app-wide one-operation lock (`crates/farhelm-ui/src/ops.rs`) is held, inputs and
Cancel are disabled (around 7616), the backdrop click is ignored (around 6079), and Launch is greyed with the tooltip
"Wait for the current operation to finish before launching" (around 138-139, 7691). The button label stays "Launch".
`on_created` (around 5839) closes the dialog and selects the new session when the reply arrives.

**Hidden launches:** quick launch and template shortcuts mount the launcher hidden behind a transparent backdrop (around
6070; `crates/farhelm-ui/assets/app.css` around 6129), so the user sees nothing until success or failure (around
4768-4777).

**Deadlines:** `REQUEST_TIMEOUT` is 60 s (`api.rs` around 1876, applied in `send_inner` around 1946) for create, restart
and replace; `READ_TIMEOUT` is 10 s (around 1916). On expiry the alert says "Not launched. " followed by the raw
transport error (`create_form.rs` around 5930-5932, 7710-7716:
`if restart_mode { "Restart error. " } else { "Not launched. " }`). The retry key is kept because the error counts as
unresolved (around 5861-5862, 5908-5915), but "unresolved" (`RefusalOutcome::Unresolved`) is not a timeout signal: it
also covers every other transport failure and most helm refusals (the helm only marks pre-dispatch failures, durable
create refusals, checkout conflicts and YOLO refusals as definitely not accepted), and the `gh:` path turns every
failure into it. Keying the new wording on it would show "outcome unknown" for real refusals. Carry a dedicated "the
deadline expired" fact from `send_inner` up to the alert instead (reqwest's `is_timeout()` works on both the web and
desktop builds), separate from `RefusalOutcome`. Plain Restart and Replace show their own error lines today (plain
Restart the bare error text, Replace "Replace: …"). The helm runs edits to completion even if the GUI hangs up
(`crates/farhelm-helm/src/middleware.rs` around 66-75), and its call to the supervisor has no timeout, so the launch
carries on and the session appears in the sidebar later while the launcher still says "Not launched.".

**Plain Restart and Replace:** `crates/farhelm-ui/src/session_view.rs` around 1345-1398 (Restart) and 1420-1441, with
`crates/farhelm-ui/src/replace.rs` around 474 (Replace). They only set flags that disable buttons, with the same 60 s
deadline.

**There are no progress events:** `/api/events` carries revision numbers only (`crates/farhelm-helm/src/events.rs`
around 1-40). Nothing in this plan changes that.

**Tests to build on** (under `e2e/tests/` unless noted): `terminal-create-idempotency.spec.ts` (inputs disabled while a
held POST is in flight, around 428; lost-reply retries around 131, 236, 571), `quick-launch.spec.ts` (a held create
reply as the hidden in-flight boundary, around 1338 and 1617), `launch-refusal.spec.ts` (the alert's shape),
`terminal-restart.spec.ts` (routes holding restart and replace), and the helper `e2e/tests/helpers/route-gate.ts`. No
test simulates a 60 s expiry today. Use Playwright's `page.clock` (already used in `e2e/tests/trash-dialog.spec.ts`): in
the web build it drives the UI's timers, the elapsed counter and the request deadline, so one held route plus clock
advances covers the threshold, the counter and the expiry without real waiting and without a product test seam. Caveats:
the existing `route.abort()` lost-reply trick cannot stand in for a timeout, and advancing the clock 60 s also expires
any other request still in flight, so hold only the request under test. Do not add a shortened-deadline seam to product
code.

## Outline

1. `plan/launch-progress/01-launcher-status`: the shared status component; the launcher footer status; the pill for
   hidden launches; the outcome-unknown wording on deadline expiry for every launcher mode, keeping "Not launched." /
   "Restart error." for real refusals; SPEC.md "Creation"; browser tests on held routes; the changelog fragment (kind
   `changed` or `added`, your call, logged).
2. `plan/launch-progress/02-restart-replace-status`: the same status and expiry wording for plain Restart and Replace;
   their SPEC passages; tests; the TODO.md entry removed.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect:

- `dprint check` on the changed files, `cargo fmt --check`, and clippy on `farhelm-ui`;
- `cargo check -p farhelm-ui --features desktop`;
- the `farhelm-ui` unit tests, through `python3 scripts/record-test-run.py`;
- `python3 releasing/check-changelog.py format`;
- `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares;
- through the recorder, on Chromium and WebKit, after the web build `.agents/validation.md` describes: the specs above
  and any new one, plus `tooltip-coverage.spec.ts` if the status adds a control.

## Fallbacks

Examples of departures that need the scope reassessment:

- any helm, protocol or supervisor change, including a helm-held list of pending launches;
- named launch steps;
- a cancel button, auto-selecting the session when it later appears, or changing the 60 s deadline;
- a sidebar placeholder row.

If the outcome-unknown promise ("will not start a second copy") does not hold for some path, word that path's message
truthfully rather than adding reconciliation; log the DECISION and name it in the report.

Known and out of scope: a dropped connection that is not a deadline expiry also leaves the outcome unknown but will
still say "Not launched.", which also falls short of SPEC.md around 3922-3924. The maintainer chose deadline expiry;
name this in the report's follow-ups rather than widening the change.

Decisions to log: the exact wording per mode, the threshold, how the pill is placed, the fragment's kind, how tests
control time, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launch-progress-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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

- A linear stack of reviewable commits, one stable bookmark and draft PR each, using the `jjstack` skill. The stack's
  base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/launch-progress/<nn>-<name>`, as the
  outline names them.
- Avoid churn: code added in one PR and deleted in a later PR of the same stack means the stack should have been shaped
  differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and the changelog fragment in the same commit as the change.
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
already made and outline). These PRs change tests, so include the full text of `.agents/test-authoring.md` verbatim in
the prompt, as root `AGENTS.md` requires.

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
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question. A
changed docs page goes there too, named by its path, in place of a preview link.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that meets the acceptance criteria above, each PR passed
its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

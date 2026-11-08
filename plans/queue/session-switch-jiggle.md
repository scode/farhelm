# Terminal characters stop jiggling when switching sessions

Written against main at e7b47815 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Jittery/jiggling characters when switching between sessions.** Switching between sessions makes characters jitter or
> jiggle: something is being re-drawn or laid out for no reason. Find what triggers the needless re-draw/re-layout on
> session switch.

What the maintainer added while planning, from a screenshot of the Mac desktop app on 0.38.0-rc.2 showing a session
running Claude Code in a new terminal (a terminal host drawn by the grid, not an old tmux terminal drawn by xterm.js):
the characters that jiggle are content in the rightmost columns of a row. Specifically, the last word of a wrapped
paragraph line that fills the terminal's full width ("you" at the far right), and the full-width horizontal rule lines
(box-drawing characters) above and below Claude Code's prompt at the bottom. Text that ends well short of the right edge
was not reported as moving.

What is true today, from a read of main at e7b47815 (line numbers approximate):

- The new terminal's view is `crates/farhelm-ui/assets/grid-terminal.js`. It measures the character cell with an
  offscreen canvas's `measureText` (falling back to a DOM measuring span) around 116-212 and 418-505, re-measures on
  font or pixel-ratio changes (`remeasure`), sets the screen width to `cols * cellWidth` around 630, and lays out runs
  of styled text from measured advances (its "Size" and "flows" comments near the top and around 656 explain how).
  Drawing is one element per visible row.
- The page's session switching and how it mounts, hides or re-attaches a terminal are in `crates/farhelm-ui` (the
  session view and `assets/terminal.js`).

So the first suspects are a measurement or layout pass that runs on switching (a re-measure while the element is hidden
or not yet laid out, a width that differs by a fraction of a pixel between passes, or the runs' fit-to-cell adjustment
being recomputed), whose effect shows most at the end of full-width rows. These are hypotheses for you to test, not
findings.

### Acceptance criteria

- Find the actual trigger of the needless redraw or relayout on session switch, and record it with evidence in the
  working log and the report.
- Fix it so switching sessions does not move already-drawn characters in new terminals, including the rightmost columns
  and full-width box-drawing lines.
- A browser test switches between sessions with full-width content (a line ending at the last column and a full-width
  box-drawing rule) and checks that the bounding boxes of the last-column text run and of the box-drawing span, as well
  as the row and screen widths, do not change across the switch, on the engines `.agents/validation.md` names, WebKit
  included (the Mac desktop app is WebKit). Box-drawing spans are sized `cells * cellWidth` (`grid-terminal.js` around
  776), so a row's box can stay fixed while its spans shift.
- If you cannot reproduce the jiggle in the test browsers on Linux, fix it from code analysis, add a test that
  discriminates on the mechanism you fixed (the switching test may pass before and after the fix there), and say plainly
  in the report that confirming the fix needs the maintainer's Mac (a devbuild, `.agents/devbuild.md`), naming what to
  look at.
- If neither reproduction nor analysis finds a convincing trigger, do not ship a speculative fix: push any diagnostic or
  test helper that is useful on its own, and block with the evidence gathered and the candidate triggers ruled in or
  out, so the maintainer can decide whether to try it on the Mac.
- Old (tmux, xterm.js) terminals are out of scope unless the same trigger demonstrably affects them through shared code.
- A `fix` PR with a changelog fragment.
- The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **A browser regression test** that switching sessions leaves rows' drawn positions unchanged, WebKit included. The
  alternative was no test.
- **If it does not reproduce on Linux, fix from code analysis** and report that confirmation needs the Mac. The
  alternative was blocking.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold**, at the maintainer's request, so it runs only once they unhold it.

## Requirement sources

**The user's request:** the TODO.md entry, the screenshot details above, and the decisions above.

**Binding repository constraints:** `docs/desktop-web-triage.md` (read it before investigating; this is a desktop UI bug
report); SPEC.md's terminal and session view sections; root `AGENTS.md`, "Tests and validation", "Conventional Commits",
"Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `.agents/narrow-tests.md`.

**Planner proposals:** the suspects listed above, and the test shape.

## Outline

One PR, `plan/session-switch-jiggle/01-no-relayout-on-switch`, typed `fix`: the fix, the browser test, the changelog
fragment, and the TODO.md removal. If the investigation needs a diagnostic or test helper that is useful on its own, it
may go first as its own PR.

### Validation

The new browser test and the existing grid terminal and session-switch browser specs on the engines
`.agents/validation.md` names, through `scripts/record-test-run.py`; `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md`. Rust checks only if Rust changes.

## Fallbacks

Examples of departures that need the scope reassessment: reworking how the grid measures cells or lays out runs in
general, changing old-terminal code, keeping hidden terminals rendered differently across the app, or changing the
terminal stream.

Decisions to log: the trigger and the evidence for it, the fix's shape and the alternatives considered, whether the
jiggle reproduced on Linux and in which engines, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-session-switch-jiggle-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/session-switch-jiggle/<nn>-<short-name>`,
  as the outline names them.
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
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

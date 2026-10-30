# Terminal tabs: find and fix what terminates a command during heavy output

Written against main at 255fb3a7 on 2026-10-30. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim as revised during planning:

> **Fix commands in terminal tabs getting terminated during heavy output.** On 2026-10-30, a long-running command run
> directly in bash in a terminal tab of an ordinary (not containerized) session, printing one line per file (tens of
> thousands of lines), was killed repeatedly two to three minutes in: it printed "Terminated", and the tab and its shell
> kept working afterwards. Other tabs were being opened and closed in the meantime, partly to debug this, so the
> tab-close lines in Farhelm's log from those moments may concern other tabs. Find out what terminated the command and
> fix it if it is Farhelm. A tab may be closed, or its processes signalled, only when its shell really exited or the
> user closed or deleted it; remove any path that decides otherwise (a timeout, slowness, anything else).

### What is known, and what is not

Facts the maintainer confirmed on the planning page:

- **The setup:** one bash, in one terminal tab of an ordinary session, ran the command directly; no multiplexer in
  between, no container.
- **What happened:** the command printed "Terminated", which is bash's report of a job killed by SIGTERM. The same tab
  and shell stayed usable; the maintainer typed further commands in it.
- **It repeated:** it happened on several runs, each two to three minutes in.
- **Other tabs:** they were being opened and closed meanwhile, some of them to debug this.

**Weak evidence, not to be anchored on:** an agent debugging on the host reported a SIGHUP "from the tab's login shell"
and pointed at Farhelm log lines from those moments:

- "tmux paused the pane for this client";
- "terminal tab closed";
- "reaped a terminal tab whose shell exited".

Those lines may concern the other tabs being opened and closed, and the maintainer explicitly warned against building on
them. Note too that the "paused" line (`crates/farhelm-supervisor/src/service/connection.rs` around 1655) is logged both
for a real tmux flow-control pause and when an attached pane dies (around 1561-1581), and names no tab.

From a read of main at 255fb3a7 and two independent planning reviews (line numbers approximate):

- **How tabs are closed:** the user's close, and the automatic reap of a tab whose panes tmux reports dead.
  - The reap path runs `ticker.rs` `reap_dead_tabs` (around 1896), then `core.rs` `try_close_tab` (around 14095), then
    `close_tab_window` (around 14183-14291).
  - `close_tab_window` does `reap_tab_tree(PaneIfLive)`, then `kill_window`, then `reap_tab_tree(MarkerOnly)`.
  - Dead means tmux's own `#{pane_dead}`, parsed strictly (`tmux/control_codec.rs` around 130-160). Nothing found
    decides death from a timeout or slowness, and a viewer that falls behind is only detached (around 1359).
- **Signal order:** a tab's process sweep (`service/sweep.rs` `kill_process_tree_with_grace`, around 1083-1177) and its
  systemd scope kill (`kill_scope`, around 1653-1665) send SIGTERM first, then SIGHUP. So a command caught in some tab's
  cleanup dies with "Terminated".
  - It is not yet known why this tab's own interactive bash survived in this incident. The executor should find out: for
    example, whether that bash was outside the selection while the command was inside it.
- **Weak spots the review found, none confirmed as the cause:**
  - The automatic close never re-checks that the tab is still dead before it reaps and kills: `close_tab_window`
    resolves the tab with dead tabs included (`terminals.rs` around 394), and `reap_tab_tree` (around 14555-14560) walks
    a live pane as a root.
  - Targeting a window or pane by a pane id that has vanished does not fail in tmux. It silently falls back to the
    session's current window and active pane (`tmux.rs` around 2020-2044). Callers of `kill_window` (around 3466) can
    hit this: Restart's old-window kill (`core.rs` around 12577), the failed-open unwind (around 14402), and
    `close_tab_window` (around 14261).
  - With tabs being opened and closed, a cleanup aimed at one tab reaching another tab's processes is a live
    possibility, through these or other paths.
- **Causes outside Farhelm are possible too,** for example something in the command's own script, or another program on
  the host. The executor rules them in or out with evidence rather than assuming.
- **Tests to start from:** `crates/farhelm/tests/e2e/terminal_tabs.rs`, especially
  `a_stalled_tab_viewer_does_not_pause_the_agents_stream` (around 1990) and its helpers, and
  `crates/farhelm/tests/e2e/terminal_backpressure.rs`.

### Acceptance criteria

- **The cause found, if it can be.** Reproduce a fast-printing command in a tab running for several minutes, both
  undisturbed and while other tabs of the same session (and of other sessions) are opened and closed, including closing
  a tab whose shell exits on its own. Use signal-level evidence (which process sent what to whom) rather than log-line
  timing. If Farhelm terminates the command, fix the cause, with the reproduction as a regression test. If the evidence
  points outside Farhelm, say so in the report with the evidence.
- **The maintainer's rule enforced everywhere:** a tab may be closed, or its processes signalled, only because its shell
  really exited or because the user closed or deleted it.
  - Audit every path that closes a tab or signals a tab's processes: the user's close, the automatic reap, Delete, Stop
    and Restart sweeps, the failed-open unwind, Restart's old-window kill, and window or pane kills that can hit the
    vanished-pane fallback.
  - Remove or fix any path that violates the rule. In particular, the automatic close must re-confirm from tmux, at that
    moment, that the tab's pane is dead before it reaps or kills anything.
  - No cleanup aimed at one tab may reach another tab's processes or window.
  - Where the rule and SPEC.md seem to conflict, follow SPEC.md, keep the behavior, and list the case in the report for
    the maintainer: the failed-open unwind, which discards a tab that never finished opening; and a containerized
    session's Stop, which ends its tabs.
- **Diagnostics, so the next occurrence is attributable from the log alone:**
  - the "paused" line names its tab and says whether tmux paused the stream or a pane death requested it;
  - an automatic reap logs the tab, its pane, and the exit status or signal tmux reported for it;
  - every tab close, and every sweep or scope kill, logs which tab it targeted and a capped list of the processes it
    signalled.
- **If the cause cannot be reproduced after a solid effort:** deliver the audit's fixes, the diagnostics, and what was
  ruled out with its evidence. Do not block.
- **Changelog and TODO:** a changelog fragment per root `AGENTS.md`, "Changelog fragments". The last code PR removes the
  TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-10-30, on the planning page:

- **The rule:** a tab may be closed, or its processes signalled, only because its shell really exited or because the
  user closed or deleted it. Any path that decides a tab is gone from a timeout, slowness or anything else is removed,
  whether or not it caused this.
- **If not reproducible:** deliver the logging, the audit (removing any path that breaks the rule) and what was ruled
  out; do not block.
- **The facts above** (ordinary session, command directly in bash, tab and shell survived, other tabs opened and closed
  meanwhile), and the instruction not to build on the debugging agent's log-line reading.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the facts and decisions above.

**Binding repository constraints:**

- SPEC.md: terminal tabs and their close and reap (around 1535-1560), flow control (around 2130-2138).
- SPEC_impl.md: flow control (around 1488-1533), the tab-close marker selection (around 3613-3623).
- Root `AGENTS.md`, "The live install is off-limits": never touch the maintainer's own Farhelm; tests make their own
  supervisors. Also "Sharing the machine with other agents", "Tests and validation", "Conventional Commits", "Changelog
  fragments", "TODO.md and FLAKES.md".
- `.agents/narrow-tests.md`, `.agents/test-authoring.md`, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- First PR, always lands: the dead re-check guard on the automatic close, the audit's other fixes, and the diagnostics.
- Second PR, only once a cause is found: the fix and its regression test.
- A long flooding run used only for investigation runs once through the test recorder; it is not added to the suite
  unless it becomes the regression test, and then with a bounded duration.

## Outline

1. `plan/tab-output-hangup/01-guard-and-diagnostics`: the re-check guard, audit fixes, diagnostics, tests for the guard
   and any fixed path.
2. `plan/tab-output-hangup/02-fix`: the root-cause fix and its regression test, the changelog fragment, and the TODO.md
   removal. If no cause is found, the fragment and TODO.md handling move to PR 1 and the report says what remains.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-supervisor`, `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py` with the
interpreter `docs/test-sleep-check.md` prepares, and, through `python3 scripts/record-test-run.py`, the supervisor unit
tests for tab close, reap and sweep, `crates/farhelm/tests/e2e/terminal_tabs.rs`, and
`crates/farhelm/tests/e2e/terminal_backpressure.rs`.

## Fallbacks

Examples of departures that need the scope reassessment:

- changing flow control or the tmux pause settings without evidence tying them to the cause;
- changing which processes a user-initiated close, Stop or Delete signals;
- removing a path the spec requires;
- adding a suite test that runs for minutes.

Decisions to log: every audited path and its verdict, the evidence for the cause (or for each ruled-out cause), every
diagnostic added, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tab-output-hangup-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/tab-output-hangup/<nn>-<short-name>`, as
  the outline names them.
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

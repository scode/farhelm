# Find out why double-clicking the version text sometimes selects nothing

Written against main at 1ba949ff on 2026-11-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Find out why double-clicking the version text sometimes selects nothing in Chromium.**
> `header controls stay interactive and text stays selectable beside the forced-class spacer` in
> `e2e/tests/window-chrome.spec.ts` failed once in a whole browser run and 1 of 20 repeats (Chromium, Tensorlake) during
> the landing of nonblocking-session-actions on 2026-11-10, which changed header CSS and added a window-level key and
> pointer capture listener. Not diagnosed, and not checked on main; first establish whether it predates that plan.

The landing's evidence is in `plans/reports/nonblocking-session-actions.report.md` (search "double-clicking the version
text"; Tensorlake shard run `580f1199`, 1 of 20). Its retained tarball's location is not recorded.

### What planning found (read from the code at 1ba949ff, not run)

**The test** (`e2e/tests/window-chrome.spec.ts` around 144-203) loads `/` at 1100x700, waits only for the version text
to be non-empty, adds the `macos-window` class and waits for the drag spacer, then calls `version.dblclick()` and reads
`window.getSelection()` at once, asserting it is non-empty and part of the version text. It never waits for a session;
the sibling test around 39-67 uses `waitForSessionRevealed`.

**Leading hypothesis.** The page selects the newest session on load (`list::view::newest_created_fallback`,
`crates/farhelm-ui/src/list/view.rs` around 617), and every test stack has a startup session (`e2e/playwright.config.ts`
around 119). When that session's terminal is revealed, `takesFocus()` (`crates/farhelm-ui/assets/terminal.js` around
5325-5330) returns true while focus is on the page body, which it is, because double-clicking plain text moves no focus.
The reveal then focuses the terminal's hidden textarea (`terminal.js` around 5270-5277; `grid-terminal.js` around 376
and 1893-1895), and in Chromium that moves the selection into the textarea. A reveal landing between the clicks or
before the selection read gives an empty selection. If this is the cause, the flake predates
nonblocking-session-actions, which may only have shifted the timing.

**What that plan changed.** #3276 (`bd2fdb7f`) and #3278 (`112fd9d8`) added the press counter
(`crates/farhelm-ui/assets/launch-activity.js` around 98-99): capture listeners on `keydown` and `pointerdown` that only
observe (no `preventDefault`, no focus move, no `user-select`), set a hidden relay input and fire a synthetic `input`
event whose Rust handler stores the count in a signal read without subscribing (`launch_activity.rs`). Its CSS touched
the sidebar rows and the launcher, not the app bar or the version text; the entry's "changed header CSS" does not hold
for `app.css`. The version text is a plain span (`crates/farhelm-ui/src/app_bar.rs` around 955-967) whose text does not
change on a test stack, and nothing re-renders it on a timer.

The revision just before that plan is `3ca47a1d` (the parent of `bd2fdb7f`).

### Acceptance criteria

1. Whether the failure reproduces is established on both `3ca47a1d` and current main, by the procedure in Decisions,
   with every attempt recorded through `python3 scripts/record-test-run.py` and every failing run retained.
2. When it reproduces, its cause is established from evidence (focus and selection events recorded around the
   double-click in failing runs, or a failing run's retained trace), not inferred, and the report says whether it
   predates nonblocking-session-actions.
3. The outcome is one of these, per Decisions:
   - the cause is the terminal taking focus: the test waits for the selected session's terminal before double-clicking,
     and the fixed test passes the same repeat count that reproduced the failure;
   - another cause a test-only change removes: that change, with the same evidence;
   - it never reproduces: a latent flake record in place of this entry.
4. The report states the procedure, the repeat counts per revision and place, the run ids, and the conclusion, including
   whether the entry's claim about header CSS held.
5. The last PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-10 planning page:

- **Where to reproduce:** locally first, 100 repeats of the test on each revision (Chromium, retries 0). If neither
  revision fails locally, run it on Tensorlake sandboxes, with at most 2 sandboxes existing at once, again 100 repeats
  per revision. This is the maintainer's request for Tensorlake that `.agents/tensorlake.md` requires, with that budget,
  for this plan only; a local failure on either revision ends the need for it.
- **If the terminal taking focus is confirmed:** fix the test only, by waiting for the selected session's terminal (the
  way the sibling test does) before double-clicking. The page's behavior (a late terminal reveal clearing a text
  selection) stays as it is; mention it in the report's "things you should know".
- **If it never reproduces on either revision, locally or on Tensorlake:** record it as a latent flake: a dated
  `FLAKES.md` entry with the fields `.agents/test-evidence.md`, "FLAKES.md", lists, a Deflake entry in TODO.md and its
  `deflake/known-flakes.txt` line (`deflake/AGENTS.md` describes both), replacing this Near term entry. Do not add the
  defensive wait in that case.
- **Reviewer:** a single fresh-context agent on Opus 5.5 at high effort (see Review gate). If it cannot be reached when
  a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled on hold**, at the maintainer's instruction on the planning page; it runs once the maintainer unholds it.

## Requirement sources

**The user's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** root `AGENTS.md`, "TODO.md and FLAKES.md", "Tests and validation", "Sharing the
machine with other agents" (a failure under shared load is an environment event, not a flake on one observation),
"Conventional Commits"; `.agents/narrow-tests.md`, `.agents/test-evidence.md`, `.agents/validation.md`,
`.agents/tensorlake.md`, `.agents/machine.md` (building an older revision outside this checkout), `deflake/AGENTS.md`
before touching `deflake/`, `.agents/test-authoring.md` for any test change.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Narrowest run, per `.agents/narrow-tests.md`: through the recorder's generic mode with the pinned tmux setup from
  `.agents/test-evidence.md`,
  `npx playwright test --project=chromium-window-chrome -g "text stays selectable" --repeat-each=100 --retries=0`. Check
  the project name against `e2e/playwright.config.ts`. Roughly 2 to 4 minutes per revision locally (an estimate).
- Build `3ca47a1d` in its own jj workspace in your scratch directory, with its own build output (`cargo build`, then the
  release `dx build`), per `.agents/machine.md`; a shared target directory is not safe for the dx build. Remove it when
  done.
- Diagnose locally with a temporary, uncommitted copy of the test that records `document.activeElement`, `focusin`
  events and the selection around each click, so a failing run shows who took focus. Only the final fix is committed.
  That copy cannot run on Tensorlake: `.agents/tensorlake.md` builds from an empty working copy and packs tracked files
  only. There, run the unchanged test (`sandbox-tests.ts` with `--grep` and `--repeat`, and `--repo <workspace>` to pack
  the `3ca47a1d` workspace) and take the evidence from the failed shard's retained `runs.tar.gz` trace.
- One local pass of 100 per revision is enough before Tensorlake; do not repeat it on a quieter machine. Under shared
  load a timing race like this is more likely to show, not less.
- The test fix needs the auto-selected session's id for `waitForSessionRevealed`: read it from the selected row (its
  `data-session-id`, say), or follow the sibling test's pattern of creating and opening its own session. Either is fine.
- If the failure reproduced only on Tensorlake, the fixed test's verification (the reproducing repeat count, at the
  place that failed) runs there too, within the same 2-sandbox budget; that is part of the agreed budget, not over it.

## Outline

One PR, its shape set by the outcome:

- Fixed: `plan/version-dblclick-flake/01-wait-for-terminal` (or a name matching the actual cause), `test:`, with the
  TODO entry removal.
- Not reproduced: `plan/version-dblclick-flake/01-record-latent-flake`, `docs:` (FLAKES.md, the Deflake entry and its
  known-flakes line, the Near term entry removed).

Neither outcome changes product behavior, so no changelog fragment.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: the reproduction runs above, the fixed test
on Chromium and WebKit (the whole `window-chrome` spec file on both, and the repeat count that reproduced on the
revision and place that failed), `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` when the test
changes, and `dprint check` on changed Markdown. Report checks run, reused and skipped.

## Fallbacks

If another plan has moved the code this file names, follow the current code and correct this file's names and line
numbers to the truth, logged as a DECISION. If main has already fixed the test or recorded the flake, the plan is done
except for whatever criteria remain; finish those.

A cause in the product rather than the test (the press counter or another page change breaking selection) is outside the
agreed fix: block with the diagnosis and the smallest fix you see as the question, after pushing nothing that assumes an
answer.

If it reproduces only on `3ca47a1d` and never on current main, the cause predates the plan and something since hid it:
diagnose it on `3ca47a1d` as above and apply the test fix on main if the cause still exists there by reading; otherwise
record the latent flake.

Examples of departures that need the scope reassessment: any product change, reworking the shared test helpers, or
spending more than the agreed Tensorlake budget.

Decisions to log: each revision's runs and verdict, the diagnosis evidence, the outcome taken, and every review finding
you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-version-dblclick-flake-log.md` in the parent directory of the checkout you run in, derived as that section
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
available as a native sub-agent type, `$scode-harness-shellout` supplies the launch mechanics.

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
  `plan/version-dblclick-flake/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- The plan's last code PR removes this plan's TODO.md entry.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high effort.
Launch it as a native sub-agent with that model and effort when your harness serves it, and otherwise through
`$scode-harness-shellout`. If it cannot be reached when a review is due and is still unreachable after a reasonable
retry, block the plan per `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is
consistent. Never substitute another reviewer.

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

The plan is complete when a linear stack of draft PRs exists in the order the outline gives, together meeting the
acceptance criteria above (as refined by Decisions already made), each having passed its review gate, with the last one
removing this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

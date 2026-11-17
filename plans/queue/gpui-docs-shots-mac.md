# GPUI last mile: capture the docs screenshots on the test Mac

written against main at 4f6cd2ad on 2026-11-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The docs screenshot process must work on the GPUI desktop app. #4050 rebuilt its runner for local Linux use
(`scripts/docs-screenshots.sh --local`, `scripts/docs_shots.py`, page modules in `scripts/docs-shots/`), reusing the
README hero's capture; on Linux its shots stop at the size check by design, because the published shots are taken on a
Mac at 2x. The README hero already has a Mac path (`scripts/readme-screenshot.sh --mac`). This plan gives the docs
screenshots the same Mac path and proves both on the test Mac, whose display was raised to 4K on 2026-11-17 so that a
1280x800 (hero) and a 1600x1000 (docs) window fit at 2x.

### Acceptance criteria

- `scripts/docs-screenshots.sh --mac <dest>` ships the checkout to the test Mac, builds and captures there under the
  Mac-wide lock with its disk check, and fetches the PNGs, reusing (or lifting into a shared helper, if that is smaller)
  the Mac half of `scripts/readme-screenshot.sh`. That transport leaves out `.git` and `.jj`, while a full docs capture
  establishes its provenance with Git and exits 3 when it cannot: keep the existing provenance check and record format
  on the originating checkout and ship only the capture work and results, with no remote clone and no new provenance
  mechanism. The proof run may use `--only launch-templates`, which does not need the complete-capture record.
- One real run of `scripts/readme-screenshot.sh --mac` and one of `scripts/docs-screenshots.sh --mac` on the test Mac
  pass their size checks (or, if the display still does not fit, the plan reports exactly what size the window came out
  at and stops there, with no window-size patch). The PNGs are not committed and not published.
- `.agents/docs-media.md`, `website/.agents/screenshots.md` and `docs/docs-shots/SPEC.md` say the docs refresh has a Mac
  path; `docs/gpui/parity/foundations.md`'s docs-shots and readme-hero entries record the Mac proof.
- While on the test Mac, check one open item and record the result in
  `docs/gpui/parity/desktop-integration-and-linux-packaging.md`: with keyboard focus on a button in the desktop app,
  does Cmd+C fall through without a system beep? Report what happened; fix nothing for it in this plan.

## Decisions already made

- **Mechanism only (maintainer, 2026-11-17).** Putting screenshots back into docs pages, re-authoring shots, the
  annotation overlay and publishing are part of a normal docs refresh, not this plan. Do not change website pages.
- **Mac rules (migration decisions, 2026-11-16 and 2026-11-17):** every Mac run takes the Mac-wide lock and checks free
  disk first; send no Apple Events to any app except System Events acting on the farhelm-desktop process (no osascript
  to Finder or anything else); a Mac result counts only from a status line the Mac side prints or the recorder's output,
  never from ssh's exit code, which the test Mac reports as 0 even on failure; no window-size patch to GPUI.
- **Permission prompts (maintainer, 2026-11-17):** if anything on the Mac shows a permission or privacy prompt, stop the
  run without clicking it, and block the plan with that as the question (the landing monitor notifies the maintainer).
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default.

## Requirement sources

**The maintainer's request:** the "GPUI last mile" TODO.md entry that points here, and the decisions above.

**Binding repository constraints:** `.agents/docs-media.md` (the flows, and that captured screenshots are never
committed on main), `docs/docs-shots/SPEC.md`, `docs/readme-hero/SPEC.md`, `website/AGENTS.md` (read before changing
`website/.agents/`), root `AGENTS.md` "Sharing the machine with other agents" and "Agent scratch space".

**Where the test Mac is and how to reach it:** not in this repository. It is described in the same `boatrunner-info.md`
file in the executing user's home directory, or by the scripts' own configuration; if neither tells you how to reach the
Mac, block with that as the question rather than guessing.

## Outline

1. `plan/gpui-docs-shots-mac/01-docs-shots-mac-path`: the Mac path for the docs runner (and a shared Mac helper if
   smaller), docs updates, the two real Mac runs, the Cmd+C check recorded, and removing the TODO entry.

### Validation

The script's own tests if it has them (`scripts/` test files run per `.agents/validation.md`), shellcheck-style care for
the shell changes, the two real Mac runs above with their results read from the Mac-side status lines, and
`dprint check` on changed Markdown.

## Fallbacks

If the 4K display still does not give a 1280x800 or 1600x1000 window at 2x, record the size it gave and finish the
mechanism anyway; that is a report item, not a block. Departures that need the scope reassessment: any change to the
desktop app itself, a new capture overlay, or publishing anything. Decisions to log: shared helper or reuse, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-gpui-docs-shots-mac-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Build and test workers

Run every cargo build, test, clippy and GUI bridge run on the shared remote build workers, not on the machine you run
on, which many agents share. How to use them (the runner, which workers executors may use, how to run tests through the
recorder there, and the rules that keep worker names out of the repository) is in the file `boatrunner-info.md` in the
home directory of the user running you. Read it in full before your first build. Never copy a worker's name, account or
any path from that file into the repository, a PR, a Blocked question or your report. Formatters and small checks that
need no build (`dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`) may
run locally.

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
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (an opus
consult or the reviewer below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/gpui-docs-shots-mac/<nn>-<short-name>`,
  as the outline names them.
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
- The last code PR of the stack removes this plan's TODO.md entry (the "GPUI last mile" bucket).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
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
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

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
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (merging or
splitting steps where the work showed a better cut, logged as a DECISION), together meeting the acceptance criteria
above, each having passed its review gate, and the last code PR removing this plan's TODO.md entry. Open, not merged. If
a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

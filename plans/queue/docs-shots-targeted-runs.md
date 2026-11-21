# Let a docs screenshot run target exactly the shots a change affects

This plan makes a docs screenshot run cheap to aim: one run can name several pages or single shots, locally and on
boats, and a pointer-parking failure says which spots were tried and why each was rejected. It covers the TODO.md entry
"Let a docs screenshot run target exactly the shots a change affects."

## The goal

The maintainer's words: "we need visual inspection to be cheap and targeted as much as possible", and, on flakiness: "If
you're having random flakiness or other issues, please bake that into the plan as well. Let's just fix things and get
these things reliable and good."

The docs screenshots (`scripts/docs_shots.py`, the page modules in `scripts/docs-shots/`, the wrapper
`scripts/docs-screenshots.sh`, and `scripts/boat/boat-tests.ts docs`) drive the real app through `scripts/gui-agent.py`.
Before each capture, `gui-agent.py capture` parks the pointer where nothing reacts to it: on the shot's own `park`
target when it names one, otherwise on the first candidate from `park_targets` that the bridge accepts a hover on. When
nothing qualifies, it fails with "no spot in the window is clear of hover and tooltip reactions to park the pointer on",
naming none of the spots it tried.

### Where it stands (from planning, at main fd102134c)

- `docs_shots.py --only` takes one page name, and so does `docs-screenshots.sh --only`. `boat-tests.ts docs` already
  takes `--only <page>` repeatedly and spreads the pages over its sandboxes.
- Within one run every shot is already attempted: `capture_pages` records each shot's failure in `failures.json` and
  goes on. In the plan that prompted this one, a launcher redesign removed the quiet title row the launcher shots parked
  on; its first staged capture (8.6 minutes) failed all nine launcher shots, and the next page was never captured
  because the executor ran one invocation per page and stopped at the first failure. The second (13.1 minutes) gave each
  shot a `park` target, and the shot that types into the search field still failed, because the results list covers that
  target. Each fix was one line; each discovery cost a staged capture.
- `park_targets` (in `gui-agent.py`) returns only the accepted candidates. The ones it filtered out, because their
  centre lies inside a drawn element flagged `hover` or `tooltip`, and the ones the bridge then refused, because
  something covers their centre, are lost.
- No docs-shot flake is on record in `FLAKES.md`.

### Acceptance criteria

1. `scripts/docs_shots.py --only` is repeatable and takes a page (`start-a-session`) or a single shot
   (`start-a-session/launcher-typed`). One run captures exactly the selected shots, in page order, attempting every one
   and reporting all failures together. A page or shot that does not exist is refused before the app starts, naming it.
   `--recapture-release-shot` keeps working as it does today.
2. `scripts/docs-screenshots.sh --only` accepts the same forms, repeated, validates them through the engine's existing
   hidden `--check-selection` rather than its own pattern and file checks, and treats any selection as a draft run the
   way a one-page run is treated today (never publishable, its capture record deleted). Clearing old output and listing
   what was captured narrow to the selected shots. A frozen release shot named with `--only` alone, which is silently
   skipped today, is refused with a message saying that `--recapture-release-shot` is the way to replace it.
3. `scripts/boat/boat-tests.ts docs --only` accepts single shots too, and `scripts/boat/sandbox/run-docs.sh`, which
   takes one page today, takes a sandbox's whole share in one call, so the app starts once per sandbox rather than once
   per page, with all of one page's shots on the same sandbox.
4. When the automatic search for a park spot fails, the error lists every candidate considered and why each was
   rejected: its centre lies in a reacting element (naming that element), or the bridge refused the hover (with the
   bridge's reason). The listing is capped at a readable length, with a count of the rest. A refused explicit `park`
   target already names the target and the bridge's reason without searching, which a test pins; that stays as it is.
5. Every instruction that documents `--only <page>` says what it now accepts, and the ones that tell agents how to draft
   or check docs shots say that a check after a GUI change runs only the affected shots, in one invocation:
   `website/.agents/refreshing.md`, `website/.agents/screenshots.md`, `website/.agents/release-notes.md`,
   `docs/docs-shots/SPEC.md`, `.agents/docs-media.md`, `.agents/boat.md`, and `.agents/validation.md` where it cites
   `--only`.
6. `scripts/test-docs-shots.py` and `scripts/test-gui-agent.py` (and the boat harness's own tests, if it has them) cover
   the selection, the refusal of unknown names and the parking report, without starting the app.
7. Any flaky or unreliable behavior you meet in the docs-shot engine, `gui-agent.py capture` or the boat docs runner
   while doing this is fixed in this plan, as its own PR in the stack when it is separate work. Only if a fix needs a
   scope reassessment's approval and does not get it is it instead recorded per `.agents/test-evidence.md`, "FLAKES.md",
   and named in the report.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **No "check parking only" mode.** A single-shot run already costs only an app start plus that shot, which is cheap
  enough.
- **Parking at a coordinate when no element qualifies is out of this plan**; it is the "Maybe later" TODO entry "Park
  the docs-shot pointer on a quiet point when no element qualifies."
- **Review gate**: a single fresh-context `gpt-6.2-sol` agent at `high` effort per PR.
- **Release note**: none needed (under "Release note" below); this tooling never ships.
- **No-workhorse mode**: the plans default, stated on the planning page.

Planner decisions:

- The selection form is `page/shot`, the names the page modules and `failures.json` already use.
- A capture that a selection did not cover is not carried over from anywhere: a selected run is a draft, as a one-page
  run is today.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the words quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Sharing the machine with other agents", "Agent scratch space", and its "Instructions by path":
  `.agents/docs-media.md` before touching `scripts/docs_shots.py`, `scripts/docs-shots/` or
  `scripts/docs-screenshots.sh`; `website/AGENTS.md` before touching `website/`; `.agents/validation.md` before touching
  `scripts/` or any test.
- `.agents/boat.md` for `scripts/boat/boat-tests.ts`. Running boats is not needed to build or test this plan and is not
  authorized by it; the boat harness's change is covered by its own tests.
- `.agents/validation.md`'s inventory and `.agents/run-all-tests.md`, updated in the same PR as any change to a check or
  test script.
- `.agents/test-authoring.md` for every test added or changed.

**Planner proposals:**

- `park_targets` keeps returning the accepted candidates; the rejected ones and their reasons come back beside them, so
  the capture can report them without searching twice.

## Outline

1. `plan/docs-shots-targeted-runs/01-docs-shot-selection`: repeatable `--only` with page and `page/shot` forms through
   `docs_shots.py`, `docs-screenshots.sh`, `boat-tests.ts docs` and `sandbox/run-docs.sh`, the refusals, tests, and the
   instruction updates.
2. `plan/docs-shots-targeted-runs/02-docs-shot-park-report`: the parking failure report, its tests, and removal of this
   plan's TODO.md entry.

Neither PR changes anything a user of Farhelm sees: a type that requires a changelog fragment gets one with `kind: none`
and the reason.

## Fallbacks

- If spreading single shots over boat sandboxes cannot keep a page's shots together without reworking the pool, run a
  shot selection on one sandbox and log it.
- If a check needs a live app run (to prove the parking report against a real tree, say), run it locally the way
  `.agents/docs-media.md` describes, or skip it, say so, and rely on the tests with recorded bridge output.
- Decisions to log: the selection syntax's edge cases, how the report is capped, and every review finding declined, with
  its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-docs-shots-targeted-runs-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-docs-shots-targeted-runs-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what the PR changes. Apply `.agents/test-authoring.md` to every
test you add or change.

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
  `plan/docs-shots-targeted-runs/01-docs-shot-selection`, `plan/docs-shots-targeted-runs/02-docs-shot-park-report`, plus
  one per separate reliability fix acceptance criterion 7 calls for, inserted where it fits.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for (`plans/.agents/executing.md`, "Executing one
  plan", step 8).
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
it targets no longer exists, record that in the working log and the report, and build only what is still missing.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (plus any
reliability fix inserted per acceptance criterion 7), together meeting the acceptance criteria above, each having passed
its review gate, with the last one removing the TODO.md entry this plan covers ("Let a docs screenshot run target
exactly the shots a change affects."). Open, not merged. If a `## Decisions` section exists, its latest entry must also
be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.

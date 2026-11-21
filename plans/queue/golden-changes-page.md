# Show the changed golden pictures on one page

This plan makes every golden screenshot run that finds changed or new pictures write one page showing only those
pictures, each as the reference, the new picture and the differing pixels, in light and dark, and describes one
visual-review procedure built on it in `.agents/validation.md`. It covers the TODO.md entry "Show the changed golden
pictures on one page." It runs after `goldens-fast-and-targeted.md`, which changes the same scripts.

## The goal

The maintainer's words: "we need visual inspection to be cheap and targeted as much as possible", and, on flakiness: "If
you're having random flakiness or other issues, please bake that into the plan as well. Let's just fix things and get
these things reliable and good."

Today a plan's visual reviewer is handed the paths of the changed pictures under `target/gui-goldens/captures` and
judges them file by file. Nothing shows what changed against the reference, and a scene that changed without anyone
meaning to is easy to miss among the pictures the change was supposed to alter. Each GUI plan also writes its own
"Visual review" section from scratch.

### Where it stands (from planning, at main fd102134c)

- `scripts/check-gui-goldens.py` compares captures with the references `scripts/gui-goldens.py fetch` writes, or, with
  `--update`, writes captures for `python3 scripts/gui-goldens.py publish`, which reports each picture as changed or
  added against the manifest `crates/gui/farhelm-gui/tests/goldens.txt`. After `goldens-fast-and-targeted.md` it also
  renders selected scenes (`--scene`) and runs several processes at once (`--jobs`); read that plan's landed result on
  main before starting.
- A compare failure leaves the new capture beside the test binary (the failure message names the directory).
- `scripts/gui-screenshot-batch.py` already reads and writes PNGs in pure Python (`read_rgba`, `write_rgba`, `crop_png`,
  with tests in `scripts/test-gui-screenshot-batch.py`), and builds a static comparison page for the old app's reference
  pictures.

### Acceptance criteria

1. A golden run, in compare or update mode, full or selected, that finds any picture differing from its reference or
   having none writes a static page with only those pictures. For each: the scene and appearance, the reference (or "new
   scene" when there is none), the new picture, and an image of the differing pixels highlighted over a dimmed copy,
   with the count of differing pixels. The run's last lines print the page's path. A run with no changes writes no page
   and removes any stale one from an earlier run.
2. The page lives in `target/gui-goldens/changes/` of the checkout running the golden check, replaced as a whole by each
   run, with its pictures copied next to it, so it can be opened on its own and handed to a reviewer as one directory.
3. In update mode, which fetches nothing today, changed and new pictures are found against the manifest the way
   `publish` finds them (`read_manifest` and `blob_id` in `scripts/gui-goldens.py`), without needing the store. Their
   references come from `FARHELM_GOLDENS_DIR` when it is set, and otherwise from the existing `fetch()` when the store
   is reachable. When neither gives them, the page shows the new pictures without references and says so at the top, and
   the run itself does not fail because of the page. The page code stays out of `gui-goldens.py`, which promises to work
   as a standalone copy.
4. Making the page does not noticeably lengthen a run, even a full `--update` after a shared component changed many
   pictures: the pure-Python decoder took about 0.9 s for one 800×830 picture on the shared host, so decode in parallel
   (a standard-library process pool) when there are many.
5. `.agents/validation.md` gets one visual-review procedure that GUI plans can point to instead of writing their own,
   rewriting the iteration paragraph `goldens-fast-and-targeted.md` added to the goldens entry rather than adding a
   second one: render only the scenes the change touches (`--scene`), give the reviewer the changes page and its
   pictures together with the design it is judged against, batch the fixes from the code review and the visual review
   and re-render the touched scenes once per round, and run one full `--update` before publishing so a shared
   component's effect on other scenes shows on the page too. It says that a scene on the page that the change did not
   mean to touch is a finding.
6. Tests cover the page generation without the app or the store: differing, new and identical pictures, the highlight
   and the pixel count, the stale page's removal, and the store being unreachable in update mode.
7. Any flaky or unreliable behavior you meet in the golden harness or `scripts/gui-goldens.py` while doing this is fixed
   in this plan, as its own PR in the stack when it is separate work. Only if a fix needs a scope reassessment's
   approval and does not get it is it instead recorded per `.agents/test-evidence.md`, "FLAKES.md", and named in the
   report.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **The page is made automatically** whenever a run finds changed or new pictures, and the run prints where it is.
- **Visual reviews use it**: `.agents/validation.md` describes the one procedure. Changing `plans/.agents/planning.md`
  so that GUI plans must cite it is not part of this plan; the maintainer is handling the plan process separately.
- **Runs after `goldens-fast-and-targeted.md`**, which changes the same scripts and adds the scene selection the
  procedure relies on.
- **Review gate**: a single fresh-context `gpt-6.2-sol` agent at `high` effort per PR.
- **Release note**: none needed (under "Release note" below); this tooling never ships.
- **No-workhorse mode**: the plans default, stated on the planning page.

Planner decisions:

- The PNG reading and writing reuses `gui-screenshot-batch.py`'s helpers, loaded as the script stands the way other
  scripts already load it, rather than adding a Python dependency, a second decoder or a new shared module.
- The page is plain static HTML with no script, in the style of the batch script's page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the words quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Sharing the machine with other agents" (no fixed path in a harness), "Agent scratch space", and its "Instructions by
  path": `.agents/validation.md` before touching `scripts/` or any test.
- `scripts/gui-goldens.py`'s rules for the store and the manifest (its module docs say where the references live and
  why); `python3 -B scripts/gui-goldens.py --self-test` covers them.
- `.agents/validation.md`'s inventory and `.agents/run-all-tests.md`, updated in the same PR as any change to a check or
  test script.
- `.agents/test-authoring.md` for every test added or changed.

**Planner proposals:**

- The page is written by `check-gui-goldens.py` after the test process (or processes) exit, from the captures and the
  references, so the Rust test only has to leave its captures where it already does.

## Outline

1. `plan/golden-changes-page/01-golden-changes-page`: the page generation, the PNG helper reuse, the update-mode lookup
   against the manifest, tests, and the inventory update.
2. `plan/golden-changes-page/02-golden-visual-review-procedure`: the visual-review procedure in `.agents/validation.md`,
   and removal of this plan's TODO.md entry.

Neither PR changes anything a user of Farhelm sees: a type that requires a changelog fragment gets one with `kind: none`
and the reason.

## Fallbacks

- If `goldens-fast-and-targeted.md` landed with different option names or a different capture layout, build on what
  landed and log the differences.
- If a highlight image would be misleading for a picture whose size changed, show the two pictures side by side with
  their sizes and no highlight, and say so on the page.
- Decisions to log: where the page lives, how the highlight is drawn, and every review finding declined, with its
  reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-golden-changes-page-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-golden-changes-page-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: `plan/golden-changes-page/01-golden-changes-page`,
  `plan/golden-changes-page/02-golden-visual-review-procedure`, plus one per separate reliability fix acceptance
  criterion 7 calls for, inserted where it fits.
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
its review gate, with the last one removing the TODO.md entry this plan covers ("Show the changed golden pictures on one
page."). Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.

## Decisions

### 2026-11-21: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-21 (claim acb5ce).
>
> ### Which screenshot comparison path should the review page use?
>
> Reviewers used to open changed screenshots one at a time, which made it easy to miss a change on an unrelated screen.
> The drafts put each changed screenshot beside its reference and highlight the differing pixels. A clean full Linux
> update captured 328 screenshots in 35 seconds and all 328 matched, so there was no page to generate. Replaying changes
> since an earlier reference set took another 28 seconds to make the page for 68 changed or new screenshots. An
> all-changed stress test took 112 seconds to produce 328 highlights. Both page timings used up to 16 processes.
>
> The plan specifies the existing Python PNG helpers and asks that a large page add little time to a full update. The
> implementation reuses those helpers, and the measured cost is substantial when many screenshots change together.
>
> Should the executor try the PNG code already compiled into the golden test executable, or keep the Python
> implementation and accept the measured wait on large reviews? I recommend trying the compiled code. It keeps the
> portable HTML page and uses an existing dependency. It needs a new entry point in the golden test executable. Approval
> would let the executor benchmark the compiled path on the same two sets. Use it if both results meet the plan’s speed
> requirement. If the timings still show a substantial delay, bring them back before landing.

The maintainer's answer:

### Answered by the landing monitor on the maintainer's behalf (2026-11-21)

The landing monitor answered this block on the maintainer's behalf, under `plans/.agents/landing.md`, "Answering
blocks": it is a reversible choice inside development tooling, not a huge question. A later change of mind is a
follow-up, not a reason to stop the plan.

Decision: use the image code already compiled into the golden test program for the page's pixel work (decoding,
comparing and drawing the highlight), as recommended. The plan's speed requirement, that making the page does not
noticeably lengthen a run, is the maintainer's; reusing the Python PNG helpers was only the planner's means to it, so
this decision overrides that planner decision. It adds no new dependency, since the test program already carries the
image library, optimized in development builds since goldens-fast-and-targeted.

- Keep the page itself plain static HTML written by the Python runner, as the plan says; only the pixel work moves.
- Prefer the cheapest place for it: a comparison run already has both pictures decoded when it finds a difference, so
  writing the highlight there costs almost nothing. Update mode needs its own entry point in the test program, in the
  style of the existing scene listing.
- Measure the compiled path on the same two sets as before (the 68 changed pictures and the all-changed 328), with the
  default number of processes. Use whichever path is faster on both, and put the timings for both paths in the report.
  Do not block again over the timings: if neither path meets the speed requirement, ship the faster one and say so in
  the report's open questions.

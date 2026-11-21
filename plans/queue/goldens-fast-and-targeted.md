# Make the golden screenshot runs fast and targeted

This plan makes the GUI's screenshot goldens cheap enough to run on every iteration: a full run in about a minute and a
half instead of eleven to twenty, a run of only the scenes a change touches in seconds, and a run that keeps going past
a broken scene. It covers the TODO.md entry "Make the golden screenshot runs fast and targeted."

## The goal

The maintainer's words: "we need visual inspection to be cheap and targeted as much as possible", and, on flakiness: "If
you're having random flakiness or other issues, please bake that into the plan as well. Let's just fix things and get
these things reliable and good."

Today `scripts/check-gui-goldens.py` runs the one libtest test `goldens::component_goldens`
(`crates/gui/farhelm-gui/src/goldens/mod.rs`), which renders every entry of `SCENES` in light and dark (328 pictures),
one after another, under Mesa's software Vulkan driver. Plans that change the GUI re-render all of them after every
round of fixes, to show a visual reviewer eight to sixteen pictures. On 2026-11-21 one plan spent 154 of its 409 minutes
in eleven such renders.

### What was measured (planning, 2026-11-21)

All numbers are from one dedicated boat VM (`large`, 8 vCPU), never from the shared development host, whose CPU is
oversubscribed: the maintainer's words, "any performance measurements are very high variance and hard to trust". On that
host the same test's libtest time ranged from 489 s to 1202 s across about sixty recorded runs. Every VM run wrote all
328 pictures the way an update run does, with a throwaway instrumented copy of the test that timed each phase per
picture.

| Variant                                      | Full run | Writing PNGs | Opening windows | Drawing until settled |
| -------------------------------------------- | -------- | ------------ | --------------- | --------------------- |
| main today                                   | 668 s    | 489 s        | 84 s            | 89 s                  |
| one app reused for every scene               | 677 s    | (same)       | (same)          | (same)                |
| image and renderer crates at `opt-level = 3` | 155 s    | 31 s         | 29 s            | 89 s                  |
| plus the UI framework and text crates        | 89 s     | 31 s         | 23 s            | 33 s                  |

- The PNG encoder ran unoptimized and every picture was encoded twice in update mode (once beside the test binary, once
  into the update directory). Opening a window builds a fresh wgpu renderer each time (the headless platform's
  `open_window` calls its renderer factory per window), so reusing one app across scenes saved nothing.
- With the image and renderer crates optimized: two processes on disjoint halves of `SCENES` took 83 s, four 44 s, eight
  33 s. With the UI crates optimized too, four took 27 s. One process used about 1.8 to 2.4 CPUs and 340 to 370 MB.
- In every variant (reused app, both optimization levels, 2, 4 and 8 shards) all 328 PNG files were byte-identical to
  main's.
- The optimized crates, by name (`[profile.dev.package.<name>] opt-level = 3`, all versions of each): `png`, `fdeflate`,
  `flate2`, `miniz_oxide`, `crc32fast`, `adler2`, `simd-adler32`, `image`; `naga`, `wgpu`, `wgpu-core`, `wgpu-hal`; and
  `gpui-pre`, `gpui-pre-wgpu`, `gpui-pre-linux`, `gpui-pre-platform`, `gpui-component`, `gpui-base`, `taffy`,
  `cosmic-text`, `swash`, `skrifa`, `harfrust`, `read-fonts`, `font-types`, `zeno`, `etagere`, `lyon`, `resvg`, `usvg`,
  `tiny-skia`. Rebuilding after the first set recompiled 32 crates and after the second 24, against a warm target
  directory. `Cargo.toml` already does this for `sha2`, with a comment saying why.
- The one recorded "never settled" failure (2026-11-21, a recipe-editor scene) was a bug in the work in progress, not a
  flake, but it showed that a scene failing to settle aborts the whole run: the `?` on `settle` (and on opening the
  window and running the setup) leaves every later scene unchecked. No golden or docs-shot flake is on record in
  `FLAKES.md`.

### Acceptance criteria

1. Development and test builds compile the crates listed above at `opt-level = 3`, through `[profile.dev.package.*]`
   entries in `Cargo.toml` with a comment that says why (the golden and GUI runs' measured times) and that shipped
   binaries are optimized anyway. Farhelm's own crates stay unoptimized. Every picture of a full update run is
   byte-identical to one from main, proven once on the boat VM and stated in the report with the run's numbers.
2. An update run writes each picture once. A compare run writes a capture only when it differs from its reference or the
   reference is missing, so a failure still leaves the new picture to look at, at the path the failure message names.
   Compare-mode captures go to a directory of this run's own under the checkout's `target/gui-goldens/`, cleared at the
   start of each run and passed to the test the way the run directories already are, never the shared build directory
   beside the test binary, so pictures from earlier runs or other checkouts never mix in.
3. A scene that fails to open, set up or settle is reported with the other failures, and the run renders every other
   scene.
4. From PR 02 on, `scripts/check-gui-goldens.py`, not the Rust test, writes the `<platform>.complete` marker, under one
   rule that never changes afterwards: the run selected no scenes and every test process exited successfully, and the
   marker holds the total number of pictures. A selected, failed or killed run leaves no marker, so
   `python3 scripts/gui-goldens.py publish` refuses its captures (that refusal is already covered by
   `gui-goldens.py --self-test`). The new script behavior gets tests of its own, since `check-gui-goldens.py` has none
   today.
5. `scripts/check-gui-goldens.py --scene PATTERN` (repeatable; shell-style wildcards matching scene names, so
   `--scene 'launcher-*'`) renders only the matching scenes, in compare and in update mode. A pattern that matches no
   scene is refused before anything renders, naming the pattern.
6. `scripts/check-gui-goldens.py --jobs N` renders on N processes at once, each a disjoint, stable share of the selected
   scenes, with a default of 4, capped at the machine's CPU count. The processes' output does not interleave unreadably
   (prefixed or collected per process). A full four-process update run's pictures are byte-identical to a one-process
   run's, proven on the boat VM, with the timings in the report. The default of 4 is the maintainer's decision and
   stays, whatever the VM shows.
7. `.agents/validation.md` describes the loop in the goldens entry: while iterating, compare or update only the scenes
   the change touches; before publishing, one full `--update` run, so a change to a shared component still shows up in
   every scene it reaches. Its inventory and `.agents/run-all-tests.md` match the script's new options in the same PR as
   each option.
8. Any flaky or unreliable behavior you meet in the golden harness or `scripts/gui-goldens.py` while doing this is fixed
   in this plan, as its own PR in the stack when it is separate work. Only if a fix needs a scope reassessment's
   approval and does not get it is it instead recorded per `.agents/test-evidence.md`, "FLAKES.md", and named in the
   report.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page, after the measurements above:

- **One plan**, built in the outline's order, because selection and parallel runs change the same code and the same
  completeness check.
- **Optimize the image, renderer and UI framework crates**, not only the image and renderer ones (89 s rather than 155 s
  per full run). The maintainer accepted that the first build after the change takes longer once, and that a debugger
  stepping into the UI framework sees optimized code.
- **Four processes at once by default**, with an option to change it.
- **One dedicated boat VM** may be used to check the speedup and the byte-identical pictures: one `large` sandbox,
  created with your own name prefix, deleted when the checks are done and before the plan closes. This is the
  maintainer's explicit permission for this plan, as `.agents/boat.md` requires.
- **Review gate**: a single fresh-context `gpt-6.2-sol` agent at `high` effort per PR.
- **Release note**: none needed (under "Release note" below).
- **No-workhorse mode**: the plans default, stated on the planning page.
- Parking the docs-shot pointer on a quiet point is a separate "Maybe later" TODO entry; docs-shot selection is the
  separate plan `docs-shots-targeted-runs.md`; the page of changed pictures is `golden-changes-page.md`, which runs
  after this plan.

Planner decisions:

- Reusing one headless app across scenes is out: measured, it saved nothing, because each window builds its own
  renderer.
- Parallel runs are separate processes, not threads, so the question of whether GPUI's headless contexts can share a
  process never arises. Build the test binary once (find it the way `scripts/run-harness-drift.py`'s `build_binary`
  does) and start N copies of it; N concurrent `cargo test` invocations would contend for cargo's build lock. Decide,
  and log, what becomes of the script's pass-through of extra cargo arguments once it runs the binary itself.
- The script owns matching and splitting. The test binary can list its scenes (an ignored listing test, say) and render
  an exact list of names it is given; the script matches the patterns with Python's `fnmatch`, refuses an unmatched one
  once, and splits the names among the processes by position (index modulo N), so each share is stable and independent
  of timing. If the listing is a test of its own, the rendering processes select theirs with `--exact`, so they do not
  run the listing too.
- The list of names and the capture directory reach the test process the way the run directories already do: variables
  in the child's environment only, built in `golden_env`. Neither the script nor any test sets variables in its own
  process's environment (root `AGENTS.md` and `.agents/test-evidence.md`).
- PNG encoder settings stay as they are: changing the compression would change every file's bytes, and publish would
  then report every picture as changed.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the words quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Sharing the machine with other agents" (no fixed path, port or name in a harness), "Agent scratch space", and its
  "Instructions by path": `.agents/validation.md` before touching `scripts/` or any test.
- `.agents/validation.md` (the goldens entry and the inventory rule) and `.agents/run-all-tests.md` (its lists), updated
  with every change to the script's options.
- `scripts/gui-goldens.py`'s rules for the store and the manifest, and its `read_captures` refusal of a platform without
  its `.complete` marker; `python3 -B scripts/gui-goldens.py --self-test` covers them.
- `.agents/boat.md` for the VM: its cost rules, owned-name rule and deletion rule.
- `.agents/test-authoring.md` for every test added or changed.

**Planner proposals:** the planner decisions above about processes, matching and splitting.

## Outline

1. `plan/goldens-fast-and-targeted/01-golden-deps-optimized`: the `[profile.dev.package.*]` entries and their comment.
   On the boat VM, one full update run from main's build and one from this PR's build, timed, with the pictures compared
   byte for byte; the numbers go in the PR description and the report.
2. `plan/goldens-fast-and-targeted/02-golden-write-once`: write each picture once in update mode and only differing or
   unreferenced captures in compare mode, into the run's own cleared directory; collect open, setup and settle failures
   per scene and keep going; the script writes the marker under its final rule; the script's first tests.
3. `plan/goldens-fast-and-targeted/03-golden-scene-selection`: the scene listing, `--scene` with `fnmatch` matching and
   the refusal of an unmatched pattern, tests, and the loop in `.agents/validation.md`.
4. `plan/goldens-fast-and-targeted/04-golden-parallel-runs`: `--jobs` with its default, the per-process output, a
   killed-process test, the byte-identity and timing check on the boat VM, the inventory update, and removal of this
   plan's TODO.md entry.

Each PR carries its own `.agents/validation.md` and `.agents/run-all-tests.md` updates for what it changes. None of
these PRs changes anything a user of Farhelm sees: a type that requires a changelog fragment gets one with `kind: none`
and the reason.

## Fallbacks

- If a crate in the list turns out not to be in the build (renamed, dropped), skip it and log it; if optimizing one
  changes any picture's bytes, drop that crate, log which and by how much it costs, and keep the rest.
- If the boat VM cannot be created (no free start, refused) after a reasonable retry, do the byte-identity checks on
  this host, which are valid anywhere, report the timings as not measured, and keep the default of 4.
- How to run the VM checks: create the one sandbox with `scripts/boat/boat-tests.ts up --count 1 --budget 1` (it handles
  the owned-name prefix, the free-start check and the bootstrap, which installs Mesa's software renderer) and remove it
  with `down`, per `.agents/boat.md`. Build the test binary locally, strip it (about 180 MB instead of 1.3 GB), copy it
  to the pool member with `boat scp`, and run it there with the environment `scripts/check-gui-goldens.py` builds, in
  update mode, so no reference pictures are needed. `boat exec` gives up after 30 seconds unless given `--timeout 0`.
  Keep the VM's work in a script file on the VM, not in a `bash -c` string.
- Decisions to log: how the scene listing is exposed, what became of the cargo-argument pass-through, the measured
  timings, every crate dropped from the list, and every review finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-goldens-fast-and-targeted-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-goldens-fast-and-targeted-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what the PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Timings taken on the shared development host are not evidence of a speedup; only the boat VM's are.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work. Record the boat sandbox's id in the log the moment it is created, so a
resumed session can find and delete it.

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
  `plan/goldens-fast-and-targeted/01-golden-deps-optimized`, `plan/goldens-fast-and-targeted/02-golden-write-once`,
  `plan/goldens-fast-and-targeted/03-golden-scene-selection`, `plan/goldens-fast-and-targeted/04-golden-parallel-runs`,
  plus one per separate reliability fix acceptance criterion 8 calls for, inserted where it fits.
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
reliability fix inserted per acceptance criterion 8), together meeting the acceptance criteria above, each having passed
its review gate, with the last one removing the TODO.md entry this plan covers ("Make the golden screenshot runs fast
and targeted."), and the boat sandbox deleted. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

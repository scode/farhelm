# Follow the desktop's live reduced-motion setting everywhere

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Use the desktop's reduced-motion setting for the attention cue and other motion.** On Linux the attention cue can
> disappear when the desktop portal and Farhelm's startup setting disagree. `layers/cue.rs` starts a flight from the
> startup value in `ticker.rs`, while GPUI can jump the flight to its transparent last frame. Follow the toolkit's
> preference everywhere and remove the separate desktop startup read. Use the stationary flash from SPEC.md's “Status”
> while reduced motion is on, including during a live change. Make ongoing cursor and status animations pause and resume
> with the preference while clocks and one-shot deadlines keep running. Test a preference change in an open window.

In product terms: remove Farhelm's separate startup-only reduced-motion check and make GPUI's live reduced-motion
preference the single source of truth everywhere. Add a small observer hook in vendored GPUI so Farhelm's animation
ticker immediately stops and restarts when reduced motion toggles live (including on macOS when the window regains
focus), switch in-flight attention cues to their stationary flash when reduced motion turns on, ensure cursor blinks,
status-dot pulses, spinners, and trash animations pause and resume cleanly (without leaving the trash icon stuck tilted)
while 30-second/1-second clocks and one-shot deadlines keep running, and test a live preference change in an open
window.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Vendored GPUI observer (`vendor/gpui-pre/src/app.rs:913-930, 1408-1419`):**
  - `App::set_reduce_motion(&mut self, reduce_motion: bool)` currently flips `self.reduce_motion` and calls
    `self.refresh_windows()`, emitting no observer callback.
  - Reuse `App`'s existing `SingleSubscriberSet<Handler>` pattern (`keyboard_layout_observers`,
    `thermal_state_observers`, `system_sleep_observers`) to add a ~15-line `App::observe_reduce_motion` hook (notified
    when `set_reduce_motion` changes the value). Record its patch file cleanly under `vendor/patches/gpui/` per
    `vendor/README.md` (`app.rs` already has the `// Modified for Farhelm:` header; add it only to any new test file
    under `vendor/gpui-pre/tests/`).
- **Desktop startup cleanup (`crates/farhelm-desktop/src/reduced_motion.rs`, `lib.rs:49, 282`,
  `crates/gui/farhelm-gui/src/bootstrap.rs:16, 266-485`, `main_view.rs:78-154`, `Cargo.toml:39-41`):**
  - Delete `crates/farhelm-desktop/src/reduced_motion.rs`, remove `Host.reduced_motion`, `Startup.reduced_motion`, and
    production `ticker::set_reduced_motion` calls (important: production startup MUST NOT call `cx.set_reduce_motion`,
    because `gpui-base` stops applying OS portal updates if the app manually writes the flag).
  - **macOS activation re-read & ordering:** `MainView` (`main_view.rs:78-154`) does not currently register an
    `observe_window_activation` subscription; `root::attach(window, cx)` (`main_view.rs:133`, `layers.rs:475-481`)
    registers the activation observer that runs `stack.cues.activate(cx)`. Register a `#[cfg(target_os = "macos")]`
    `observe_window_activation` subscription in `MainView::new` **before** `root::attach(window, cx)` (guarded with
    `if window.is_window_active()`), calling `gpui_base::apply_system_reduce_motion(cx)` (verified cross-platform and a
    no-op under `TestAppContext` while respecting `demo_video.rs`'s explicit override), so pending cues see the updated
    macOS preference before `layers.rs` starts them.
  - Update the stale `# Not used directly.` comment above `gpui-base.workspace = true` in
    `crates/gui/farhelm-gui/Cargo.toml:39-41`.
- **Ticker (`crates/gui/farhelm-gui/src/ticker.rs`):**
  - Remove `Ticker.reduced_motion`; have `ticker::reduced_motion(cx)` return `cx.reduce_motion()` directly, and have
    `Blinking::is_on`, `Pulsing::step`, `subscribe`, and `tick` check `cx.reduce_motion()`.
  - Subscribe to `cx.observe_reduce_motion(...)` in `ticker::install` and **store the returned `Subscription` on the
    `Ticker` global** (mirroring `QuitWiring` in `bootstrap.rs:349-352`, rather than detaching it): turning reduced
    motion ON immediately cancels `tick_loop` (keeping clocks and `ticker::after` deadlines running unaffected); turning
    reduced motion OFF immediately restarts `tick_loop` if any live `animating` subscriber exists.
  - Keep a 3-line
    `#[cfg(test)] pub(crate) fn set_reduced_motion(reduced: bool, cx: &mut App) { cx.set_reduce_motion(reduced); }`
    wrapper so production code can never override `gpui-base` while avoiding mechanical churn across ~15 unrelated test
    files.
- **Attention cue & other motion consumers:**
  - `crates/gui/farhelm-gui/src/layers/cue.rs:55, 75-101, 131`: consult `ticker::reduced_motion(cx)` in `Cues::draw(cx)`
    so that turning reduced motion ON mid-flight immediately draws the stationary flash (`Frame::Flash(0.24)`) instead
    of GPUI jumping `cue_progress` to transparent `Frame::Flash(0.0)` (latching `moving = false` on `false -> true`
    avoids restarting a fresh flight if toggled back within the same 1050 ms window, or keep stationary flash cleanly
    without extra machinery). If reduced motion turns OFF mid-stationary-flash, keep the stationary flash for its
    remaining <650 ms rather than starting a truncated comet flight.
  - `crates/gui/farhelm-gui/src/trash.rs:766-787`: guard `wiggle` (and `bob_lift`) on `!ticker::reduced_motion(cx)` so
    toggling reduced motion ON mid-wiggle drops the wiggle handle and snaps the trash icon upright instead of freezing
    it tilted.
  - `crates/gui/farhelm-gui/src/components/tests/transition.rs:241-249`: update the test that previously asserted
    `ticker::reduced_motion(cx) && !cx.reduce_motion()` now that both share one flag.
  - Update stale comments documenting the old startup-only `Host.reduced_motion` split in
    `crates/gui/farhelm-gui/src/components/transition.rs:28-33, 164-166`,
    `crates/gui/farhelm-gui/src/components/spinner.rs:48-50`, `bootstrap.rs:16`, `ticker.rs`, and
    `SPEC_impl.md:1323-1394` ("Animation time: the ticker").

### Acceptance criteria

1. `crates/farhelm-desktop/src/reduced_motion.rs` and `Host.reduced_motion` / `Startup.reduced_motion` /
   `Ticker.reduced_motion` are removed; `ticker::reduced_motion(cx)` reads `cx.reduce_motion()` everywhere, and
   production startup never overrides `gpui-base`'s portal ownership.
2. Vendored GPUI exposes `observe_reduce_motion` (reusing `SingleSubscriberSet<Handler>`, with its vendor patch in sync
   per `vendor/README.md`), and `ticker::install` stores that `Subscription` on `Ticker` to immediately pause
   `tick_loop` when reduced motion turns ON and resume `tick_loop` when reduced motion turns OFF while live animating
   subscribers exist. Clocks (30 s / 1 s) and `ticker::after` deadlines keep running regardless.
3. On macOS, `MainView` registers an activation observer **before** `root::attach(window, cx)` calling
   `gpui_base::apply_system_reduce_motion(cx)` when `window.is_window_active()` so Reduce Motion changes in System
   Settings take effect live before pending attention cues activate.
4. `layers/cue.rs` renders the stationary flash whenever reduced motion is on, including during a live `false -> true`
   change mid-flight; `trash.rs` clears active wiggle/lift on reduced motion instead of freezing tilted.
5. Tests exercise a live `cx.set_reduce_motion(true)` / `false` toggle in an open window (covering attention cue
   stationary flash, cursor blink and status pulse pausing and resuming, and clocks/deadlines continuing) plus the
   required vendored `gpui-pre` test suite. `SPEC_impl.md` and stale comments are updated and the TODO.md entry is
   removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Add a tiny observer hook in vendored GPUI (`set_reduce_motion` notifies subscribers) so the ticker stops and restarts
  immediately everywhere.
- On macOS, re-read macOS's Reduce Motion setting on window activation too (`gpui_base::apply_system_reduce_motion`).
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `vendor/README.md`: keep the new GPUI patch file and `vendor/gpui-pre/` in sync and verify clean application.
- `.agents/validation.md:629-635`: run the vendored `gpui-pre` test suite whenever anything under `vendor/gpui-pre*` or
  `vendor/patches/gpui/` changes.
- `SPEC.md:2932-2936` ("Status") and `SPEC_impl.md:1323-1394`.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Keep `layers/cue.rs` `true -> false` mid-stationary-flash staying stationary for its remaining <650 ms (avoids a
  clipped comet flight), and keep `false -> true` mid-flight switching to stationary flash until its original expiry.
- Keep a 3-line `#[cfg(test)]` `ticker::set_reduced_motion` wrapper around `cx.set_reduce_motion` to avoid mechanical
  churn across ~15 test files while keeping production code off the setter.
- Slice into 2 PRs: (1) vendored GPUI `observe_reduce_motion` patch + test + `vendor/README.md`, (2) remove desktop
  startup read, unify `ticker.rs` + `layers/cue.rs` + `trash.rs` + macOS activation re-check + live-toggle tests +
  `SPEC_impl.md` + stale comments + TODO removal.

## Outline

1. `plan/toolkit-reduced-motion/01-gpui-observe-reduce-motion`: add `App::observe_reduce_motion` in
   `vendor/gpui-pre/src/app.rs` (reusing `SingleSubscriberSet<Handler>`, fired from `set_reduce_motion` when the value
   changes), add a unit test in `vendor/gpui-pre`, add its patch file and entry in `vendor/README.md`, and verify clean
   patch application. `feat: notify observers when GPUI reduced-motion preference changes` (with `kind: none` changelog
   fragment — internal toolkit hook used by the next PR).
2. `plan/toolkit-reduced-motion/02-unify-live-reduced-motion`: delete `crates/farhelm-desktop/src/reduced_motion.rs`,
   remove `Host.reduced_motion` / `Startup.reduced_motion` / `Ticker.reduced_motion`, subscribe `ticker` to
   `observe_reduce_motion` (storing the `Subscription` on `Ticker`) to pause/resume `tick_loop` live (leaving clocks and
   deadlines running), register `#[cfg(target_os = "macos")]` activation re-read in `MainView::new` before
   `root::attach`, fix `layers/cue.rs` and `trash.rs` live-toggle handling, update `components/tests/transition.rs` and
   add open-window live-toggle tests, update `SPEC_impl.md` and stale comments (`Cargo.toml`, `transition.rs`,
   `spinner.rs`, `bootstrap.rs`), add a `kind: fixed` changelog fragment, and remove the TODO.md entry ("Use the
   desktop's reduced-motion setting for the attention cue and other motion").
   `fix: follow the desktop's live reduced-motion setting everywhere`.

## Fallbacks

- None expected; escalate via `queue block` only if a repository invariant conflicts with the settled behavior above.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-toolkit-reduced-motion-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-toolkit-reduced-motion-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run the required vendored `gpui-pre` test suite (`.agents/validation.md:629-635`:
`cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support --test native_key_metadata --test driver_record --test hitbox_occlusion`,
plus any new `observe_reduce_motion` test), targeted tests in `farhelm-gui` (`ticker_tests`, `layers::cue`,
`components::tests::transition`, `trash`, `terminal::attached::tests`, `session_list`), `farhelm-desktop` build/check,
vendor patch verification per `vendor/README.md`, `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`,
`dprint check` on changed Markdown, and `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares
it. Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the
live Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Use the desktop's reduced-motion setting for the
  attention cue and other motion"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

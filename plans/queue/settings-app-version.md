# Show the version, with Check for updates, in Settings → App

Written against main at 54f5f8cb1 on 2026-11-18. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Settings → App gains a **Version** row at the top: the running Farhelm version, a **Check for updates** button that runs
the same on-demand check as the sidebar's version readout, and a help line. The TODO.md entry this plan covers (Near
term → Trivial), in the maintainer's words:

> **Show the version in Settings > App.** add the current version of farhelm in the settings -> app dialog in addition
> to the header of the app (user feedback, someone looked for it there).

### What the PR does

One `feat:` PR, `plan/settings-app-version/01-version-row`:

- In `crates/gui/farhelm-gui/src/settings/app.rs`, the section draws, after its existing note and before the switch
  group, a Version row laid out like the dialog's switch rows: the name **Version** on the left at the switch label's
  size, and on the right the running build's version (`farhelm_proto::BUILD_VERSION`, the same text the sidebar's top
  bar shows) in the monospace face, followed by a **Check for updates** button. Under the row goes a help line, for
  example "The Farhelm release this app is running."
- Pressing the button calls `crate::desktop::updates::check_now`, the one route every on-demand check already uses.
- While the updater reports a check or an install in progress (its readout's `Activity::Checking` or
  `Activity::Installing`, whichever route started it), the button reads **Checking…** or **Installing…** and cannot be
  pressed. Otherwise it reads **Check for updates**. The words come from the readout's existing presentation
  (`update_readout::InlineStatus::text`), not new literals. The result of a check ("Up to date", a failure, an update
  ready) still shows only where it does today, at the top bar's version readout. The button's hover help says so.
- The open dialog redraws when the updater's readout changes, so the button follows a check that starts or ends while
  Settings is open.
- SPEC.md, "Session list", the paragraph describing the **App** section (it starts "The installed desktop app's dialog
  also has an **App** section"): it gains the version row and its button. SPEC.md, "Installation and updates", the
  sentence listing the on-demand routes ("The update can be checked for on demand by selecting the version, ..."): it
  gains Settings → App's **Check for updates**, with the button's busy state stated once.
- The docs: `website/src/content/docs/docs/using/settings.md`, "## App", mentions the version and the button;
  `website/src/content/docs/docs/using/update-and-uninstall.md`, "## Check for an update now", lists the new route, and
  "## Turn automatic updates off" too if its sentence naming the routes would otherwise read as complete.
- Module docs: `crates/gui/farhelm-gui/src/desktop/updates.rs` lists Settings → App among `check_now`'s routes;
  `settings/app.rs`'s module docs describe the section's new shape (they currently describe it as only the switch).
- Tests in `crates/gui/farhelm-gui/src/settings/tests.rs`, beside the three existing App tests: the row shows the build
  version; pressing the button runs one check; while the updater reports Checking or Installing the button shows those
  words and a press runs no check.
- The `settings-app` screenshot golden (`crates/gui/farhelm-gui/src/goldens/settings.rs`) is re-captured, and its
  docstring updated for the new row.
- The TODO.md entry "Show the version in Settings > App." is removed.
- A changelog fragment under `releasing/changelog.d/`, `kind: added`, saying what users see (root `AGENTS.md`,
  "Changelog fragments").

### Acceptance criteria

- In a desktop app whose updater runs, Settings → App shows, above the automatic-updates switch and below the section's
  note, a Version row with the running version in the monospace face, a **Check for updates** button, and a help line.
- Pressing the button runs exactly one on-demand check through `desktop::updates::check_now`. While any user-visible
  check or install is in progress, the button reads **Checking…** or **Installing…** and a press does nothing.
- Which apps list App is unchanged. Nothing new in SPEC.md, the docs or the code says the row is specific to the Mac
  (see Decisions).
- SPEC.md, the two docs pages and the module docs above describe the row and the new route.
- The tests above pass. The `settings-app` golden is re-captured and passes. `dprint check` passes on the changed files.
- The TODO entry is gone, and the fragment passes `python3 releasing/check-changelog.py format`.

## Decisions already made

All from the maintainer on the 2026-11-18 planning page, which showed mockups of each option drawn in the real dialog's
look.

- **Placement: a Version row at the top of the section.** Chosen over a quiet "Farhelm 0.41.0" line under the switch.
  The row is the first thing below the section's existing note ("This one belongs to this Farhelm app on this Mac."),
  laid out like the dialog's switch rows (name left, value where a switch would sit), with a help line under it.
- **The row carries a Check for updates button.** Chosen over plain text, and over a second copy of the top bar's full
  readout (red when an update is ready, its menu). The button runs the existing on-demand check.
- **While a check runs, the button shows it** (follow-up question): **Checking…** or **Installing…**, not pressable,
  then back to **Check for updates**. The result stays at the top bar's version, which the button's hover help says.
  Rejected: hover help alone (a press in the dimmed window could look like nothing happened), and Settings showing "Up
  to date" or "Check failed" itself (a second copy of the readout's timing and dismissal rules).
- **Which apps get it: only where App already exists**, that is, a desktop app whose updater runs. The maintainer's
  words: "because its the only one right now. when we add linux support we'll add it there too. don't spec this as
  intentinoally mac specific, we just dont support linux yet." So SPEC.md keeps App's condition as it is ("a desktop app
  whose updater is active") and the new text adds no Mac-specific wording. Existing Mac wording elsewhere (the section's
  note, the docs' "In the installed Farhelm app on your Mac") describes today's only updater and stays as it is.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No release note:** "No release note needed." The PR still leaves an ordinary `kind: added` fragment, so the release
  curates it into the folded full list (`releasing/.agents/changelog.md`, "Plans' release notes").
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Builds and tests on the executing machine,** not the boat sandboxes; stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry quoted under The goal, and the Decisions above.

**Binding repository constraints:** SPEC.md, "Session list" (the settings dialog and its **App** section) and
"Installation and updates" (the on-demand check routes and the version readout); root `AGENTS.md`, "Conventional
Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md"; `.agents/validation.md` and
`.agents/test-authoring.md` for the tests and the golden.

**Planner proposals** (change them if the code reads better otherwise, logged as a DECISION):

- The row is a local flex row in `settings/app.rs`, using `choices::LABEL_TEXT` and `LABEL_LINE_HEIGHT` for the name,
  and noted in its docstring as matching the switch row's geometry. Do not lift a generic "label with a trailing
  control" row out of `components/checkbox.rs` for this one caller.
- The version value reuses the header's plain-version element (`StatusMark::new(...).monospace()`, as
  `sidebar/header.rs` draws it when no updater runs) under its own element id, or a plain monospace text element if that
  reads better. It never reuses the readout's private version drawing in `update_readout/view.rs`, whose colors belong
  to the top bar's control.
- The button is the shared `components::Button` at `Tier::Neutral`, as Settings already uses it
  (`choices::retry_loading` is an example), with hover help along the lines of the `?` menu item's: it checks now and
  installs a newer release, and the version at the top shows how it went.
- The section's return shape grows from two pieces to three; no new abstraction for that.
- `HELP_OFF` in `settings/app.rs` (the switch's off-state help) lists the on-demand routes ("from the ? menu or this
  machine's update item") and is drawn right under the new button. Extend it to name the button, and update the test
  that pins its exact text.
- Opening Settings at App keeps focusing the automatic-updates switch, as now (`SettingsDialog::start`), and the
  `settings-app` golden keeps its focus setup.
- The tests reuse the shared fake updater (`desktop/updates/fake.rs`, whose `Seen::checks` counts checks, as
  `update_readout/view/tests.rs` and `hosts/menu/tests.rs` assert): change the settings tests' `keep_updater` helper to
  return or accept its `Rc<Seen>`, and set the fake's published readout to Checking or Installing for the busy-state
  test. Do not copy `help_menu/tests.rs`'s hand-built counting updater.

## Outline

One PR: `plan/settings-app-version/01-version-row`, everything under "What the PR does" above.

The work extends the App section (`settings/app.rs`, wired through `Section::App` in `settings.rs`), and reuses the
updater's existing surface (`desktop::updates::{runs, entity, readout, check_now}`), the readout's status words
(`update_readout::InlineStatus`), and the shared `StatusMark` and `Button` components. It adds no updater state, no new
check route and no status display of its own.

### Validation

Per `.agents/validation.md`, choose the narrowest runs that cover the change, every runtime test through
`python3 scripts/record-test-run.py`: the `farhelm-gui` settings tests (the App tests and anything else in
`settings/tests.rs` the helper change touches), the `settings-app` golden through `scripts/check-gui-goldens.py` as
`.agents/validation.md` describes, `python -B scripts/check-test-sleeps.py` with the isolated interpreter from
`docs/test-sleep-check.md` (Rust tests change), `dprint check` on the changed files, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

Examples of departures that need the scope reassessment: showing a check's result ("Up to date", "Check failed") in
Settings; sharing or copying the version readout's feedback state (its `InlineFeedback`, its timers); adding a new check
route or updater state; listing App in apps without an updater; adding a Mac-specific statement for the row.

Decisions to log: each planner proposal you change, the final wording of the help line, the hover help and `HELP_OFF`,
and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-settings-app-version-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-settings-app-version-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests on the machine you run on, as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`. The maintainer chose this on 2026-11-18; do not use the boat sandboxes
(`.agents/boat.md`) for this plan.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (one of its remotes
points at that GitHub repository; if not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill
with the log file's absolute path. If the log exists, read it and resume where the previous session left off,
cross-checking it against reality (the jj graph, bookmarks and open PRs) rather than starting over. If it does not
exist, this is a fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the
resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

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

- One reviewable commit, stable bookmark and draft PR, as the outline names it, using the `jjstack` skill. The stack's
  base is `main@origin`, or this plan's own open PR when it resumes, set up per `plans/.agents/executing.md`, "Executing
  one plan" (step 6); never another plan's PRs. The bookmark is `plan/settings-app-version/01-version-row`.
- Keep the PR bite-sized. Within this run, a PR that needs correcting is restructured rather than corrected on top, and
  that applies to a PR an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit (root `AGENTS.md`, "Changelog fragments"), whatever this
  plan's Release note section says.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- The PR removes the TODO.md entry it covers (`plans/.agents/planning.md`: a TODO plan's last code PR removes its
  entries).
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
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
already made and outline). Because the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim
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

The plan is complete when the draft PR the outline names exists, meets the acceptance criteria above, and has passed its
review gate. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.

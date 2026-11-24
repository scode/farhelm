# Drift tests: permission modes and custom permission flags

Written against main at 423770e10 on 2026-11-24. Depends on `drift-model-selection.md` having landed on `main` first
(which introduces `--category` runner support and modularizes `crates/farhelm/tests/e2e/real_agent_capture/drift/`).
This is a goal file for an unattended run, executed through the planning system in `plans/AGENTS.md`. You, the executing
agent, have none of the conversation that produced it; everything you need is here or in the repository files it names.
SPEC.md and SPEC_impl.md stay authoritative over anything this file says, and where this plan changes what they specify,
it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Drift test feature coverage"). Verbatim:

> **Drift tests: permission modes and custom permission flags.** Cover YOLO launch flags on Claude, Codex, Antigravity,
> Grok, Muse, and OMP (currently only Guarded is exercised for those six); Goose's `approve`, `smart_approve`, and
> `chat` modes (`GOOSE_MODE`); OMP's `Approve` mode (`--approval-mode always-ask`); Cursor's "Agent's own setting"; and
> custom permission flags surviving Restart (including Antigravity's `--settings` and `--` argument ordering).

The maintainer's general direction across this batch of drift-test plans:

> the general idea here is: add the test category to our drift tests, plug in each harness. for each harness, make a
> reasonable effort to get it plugged in, and fix issues where it's straight forrward. if a harness cannot be plugged in
> easily, or it plugs in but there are bugs to fix that are not straight forward fixes, then create near term todo item
> to fix those issues but do not scope creep this plan to fix everything.

(The maintainer then settled that deferred TODO entries go in `## Plan follow-ups`, matching `plans/AGENTS.md`, and
confirmed dropping Cursor's retired "Agent's own setting" check because `remove-own-setting-permission` removed that
mode.)

### Acceptance criteria

1. **Permission modes and custom flags drift category:**
   - Add `crates/farhelm/tests/e2e/real_agent_capture/drift/permission_modes.rs` and wire `mod permission_modes;` in
     `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs`.
   - Register ignored `drift_<harness>` cases via the shared category registration macro for the 8 harnesses that offer
     non-YOLO or Custom modes (Claude, Codex, Antigravity, Grok, Muse, OMP, Goose, Cursor; Pi and OpenCode are YOLO-only
     and already run in YOLO mode in `base`), pinned with a compile-time or unit guard against
     `LaunchHarness::BUILT_INS` (`!offers_only_yolo()`).
   - Cover per harness:
     - Claude, Codex, Antigravity, Grok, Muse, OMP: YOLO launch flags (`--dangerously-skip-permissions`, `--yolo`,
       `--always-approve`, `--approval-mode yolo`), verifying CLI acceptance and live `SessionInfo.screen_yolo == true`
       through `client.list_sessions()` where Farhelm's screen reader classifies that harness's YOLO footer (Claude,
       Grok, Muse).
     - Goose: `approve`, `smart_approve`, and `chat` modes (`GOOSE_MODE=...`), verifying CLI acceptance for each.
     - OMP: `Approve` mode (`--approval-mode always-ask`), verifying CLI acceptance.
     - Cursor: current YOLO (`--force`, plus `SessionInfo.screen_yolo == true` if detected) and Custom permission flags
       (launch-only). Drop retired "Agent's own setting".
     - Custom permission flags surviving Restart: across all tracked harnesses that offer `LaunchPermission::Custom`
       (Claude, Codex, Antigravity including `--settings` and `--` argument ordering, Grok, Muse, OMP), launch with
       valid custom permission flags, complete a turn, Restart, verify `resume_argv` preserves the custom permission
       flags (and Antigravity's argument ordering), and confirm the resumed CLI starts cleanly.
   - Keep this category bounded to permission modes and Custom Restart survival; do not absorb the separate TODO entry
     ("Drift tests: permission-mode status-line footer detection") or trigger real tool calls to test approval dialogs.
2. **Live verification, straightforward fixes, deferrals, and TODO cleanup:**
   - Run `python3 -B scripts/run-harness-drift.py --no-upgrade --category permission_modes` live against the installed
     CLIs on the executing host.
   - Fix straightforward issues discovered during live runs (adding unit tests and a changelog fragment if a user-facing
     fix lands in production code); defer any harness that cannot be plugged in easily or reveals non-trivial bugs into
     `TODO.md` under `## Plan follow-ups` without scope-creeping.
   - Remove the covered `TODO.md` entry (`Drift tests: permission modes and custom permission flags.`) in the final PR.

## Decisions already made

### Maintainer decisions (binding)

- **Queue ordering:** Runs `(after drift-model-selection.md)` using the `--category` runner and modular `drift/` layout
  it introduced.
- **Retired Cursor "Agent's own setting":** Drop it; cover Cursor's current YOLO and Custom permission modes only.
- **Verification depth:** Verify clean CLI acceptance, rendered permission indicators where shown (reusing Farhelm's
  existing ticker/status-line reader), and Restart preserving Custom flags (with real turn + Restart on tracked
  harnesses); do not prompt tool calls to trigger approval dialogs.
- **Plugging in harnesses & deferral rule:** Make a reasonable effort to plug in each supporting harness and fix
  straightforward issues; defer hard-to-plug harnesses or non-trivial bugs to `## Plan follow-ups` in `TODO.md`.
- **Live validation during execution:** Run `--no-upgrade --category permission_modes` against the currently installed
  CLIs on the executing host (no vendor upgrades during execution).
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- `AGENTS.md`: Never mutate process environment variables in tests; never touch the live Farhelm install; annotate every
  test sleep with `// sleep-ok:` passing `scripts/check-test-sleeps.py`.
- `docs/harness-drift-tests.md`: Drift cases remain `#[ignore = "manual live vendor drift check"]` and harness names
  must match `scripts/harness_upgrades.py`'s `RECIPES` keys.
- `crates/farhelm-proto/src/launch.rs`: `LaunchHarness::offers_permission` and `offers_only_yolo` are the authoritative
  sources of which permission modes each harness accepts.
- `crates/farhelm-helm/src/launches.rs`: Antigravity's `workspace_trust_blocked_by_selection` and
  `trust_custom_tail_len` govern how `--settings` and `--` interact with Custom permission flags.

### Planner-chosen implementation shape

- **Reuse `SessionInfo.screen_yolo` via `client.list_sessions()`:** `serving_supervisor` already runs the live ticker
  that calls `screen_reader::status_line_yolo` and populates `SessionInfo.screen_yolo`. Poll `client.list_sessions()`
  for YOLO indicator checks instead of adding a speculative `test-seams` export for `status_line_yolo` (which also
  avoids conflicting with sibling plans on `agent_kind/mod.rs`).
- **Keep `drift/mod.rs` edits additive:** Add only `drift/permission_modes.rs` and one `mod permission_modes;` line in
  `drift/mod.rs`. Reuse `drift/mod.rs`'s launch/teardown helper, `Terminal`, `wait_for_turn`, and category registration
  macro.

## Outline

### Existing mechanisms reused

- `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs` (launch/teardown scaffolding, `Terminal`, `wait_for_turn`,
  category registration macro).
- `crates/farhelm-helm/src/lib.rs` (`compile_agent_launch`).
- `crates/farhelm-proto/src/launch.rs` (`LaunchHarness::BUILT_INS`, `offers_permission`, `offers_only_yolo`,
  `LaunchPermission`).
- `SupervisorClient::list_sessions` (`SessionInfo.screen_yolo`) and `restart_session`.

### PR stack

1. **`plan/drift-permission-modes/01-permission-modes-drift-cases`** — Add `permission_modes` drift category across the
   8 harnesses with non-YOLO or Custom modes:
   - Add `crates/farhelm/tests/e2e/real_agent_capture/drift/permission_modes.rs` and declare `mod permission_modes;` in
     `drift/mod.rs`.
   - Plug in Claude, Codex, Antigravity, Grok, Muse, OMP, Goose, and Cursor covering YOLO flags, Goose's
     `approve`/`smart_approve`/`chat`, OMP's `Approve`, Cursor's YOLO + Custom, and Custom flags surviving Restart
     (including Antigravity `--settings` and `--` ordering).
   - Run `python3 -B scripts/run-harness-drift.py --no-upgrade --category permission_modes` live; fix straightforward
     issues, defer non-trivial ones into `TODO.md` under `## Plan follow-ups`, and remove the covered `TODO.md` entry.

## Fallbacks

- If an installed agent CLI lacks login or is not installed on the executing host (`COULD NOT RUN`), keep its case wired
  cleanly and report the exact `COULD NOT RUN` detail in the working log and final report.
- If a vendor's YOLO footer wording drifted so `SessionInfo.screen_yolo` does not latch `true`, check whether the fix in
  `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs` is straightforward (with a fixture test) or non-trivial;
  if non-trivial, defer it to `## Plan follow-ups` with the observed CLI version and footer text.
- Scope-reassessment triggers (examples, not a blacklist): adding tool-execution approval probing, rewriting
  `screen_reader.rs`, or pulling in the separate status-line footer detection TODO entry.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-drift-permission-modes-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-drift-permission-modes-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- PR 1:
  - `python3 -B scripts/test-run-harness-drift.py` (through `record-test-run.py`).
  - `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'drift module unit tests' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`.
  - `python3 -B scripts/run-harness-drift.py --list --category permission_modes`.
  - Live verification on currently installed CLIs:
    `python3 -B scripts/run-harness-drift.py --no-upgrade --category permission_modes` (record the resulting run IDs and
    verdicts in the working log and final report).
  - `cargo fmt --all -- --check`, `cargo clippy -p farhelm --all-targets -- -D warnings`,
    `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, `dprint check` on changed Markdown, and
    `python3 releasing/check-changelog.py format` if any changelog fragment was added.

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Drift tests: permission modes and custom
  permission flags."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred harness issues added to
  `TODO.md` go under `## Plan follow-ups`.
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
alternatives considered: in particular those named under Planner-chosen implementation shape and Fallbacks above, every
deferred harness/bug recorded in `## Plan follow-ups`, and every review finding you decided not to follow. The
maintainer will ask for these later.

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

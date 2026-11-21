# Real-agent harness behavior drift test suite

Written against main at a5157ee92 on 2026-11-20. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Two TODO.md entries (bucket "Near term", subsections "Tensorlake" and "Trivial"), folded into one plan at the
maintainer's request, verbatim:

> **Harness behavior drift tests.** A suite, run semi-regularly rather than in every check run, that starts each real
> agent harness Farhelm supports (Claude, Codex, Antigravity and the rest), as currently installed, and checks the
> behavior Farhelm depends on: launch flags, conversation reports through the hook, the report checks Farhelm applies to
> them, Restart and resume. The point is to notice when an upstream update breaks Farhelm, before a user does. Today
> nothing does this: the tests drive stand-in agents and recorded transcripts, `scripts/agent-tui.py` is a manual tool,
> and re-capturing agent screens (`docs/agent-screen-fixtures.md`) is a manual job that only covers status reading. It
> spends real vendor turns, so it needs logins and a budget, and stays out of the default test run.

> **Test that a real Antigravity still gets its conversation reports accepted.** To keep Restart on the main
> conversation rather than a subagent's, Farhelm now accepts a conversation report from Antigravity only when the
> conversation's log starts the way a main conversation's did in Antigravity CLI 1.5.13 on Linux. Every test of that
> check uses recorded logs, so if a later Antigravity writes its logs differently, Farhelm silently ignores every new
> report and Restart keeps opening an old conversation, and no test notices. Add a test that runs the installed
> Antigravity CLI, sends a short prompt, and checks that Farhelm accepts the report; it spends a real turn, so it
> belongs with the harness drift tests (Near term) rather than in the default run. Raised by the
> antigravity-subagent-check plan, 2026-11-20.

### Where it stands (from planning, at main a5157ee92)

- `crates/farhelm/tests/e2e/real_agent_capture.rs` holds four `#[ignore]`d Rust e2e tests covering Claude Code and Codex
  (conversation capture and `/clear` or `/new`), running each real CLI inside a real supervisor and terminal host with
  grid attachments and screen polling (`screen`, `StreamText`, `wait_for_agent_tui_ready`). Its
  `reported_runtime_identity` and `resume_identity` helpers (lines 380–405) currently parse only `codex:` and plain
  conversation IDs, not `pi:`, `omp:`, or `grok:` locators.
- `farhelm_helm::compile_agent_launch` (`crates/farhelm-helm/src/lib.rs:188`) is already `pub` for e2e tests to compile
  production launch commands and resume commands for any `LaunchHarness`.
- Six conversation-tracked harnesses (Claude, Codex, Muse, Goose, Pi, OMP) inject their hook or extension via
  `{farhelm_args}` pointing directly at `farhelm_bin()`. **Antigravity** (`~/.gemini/config/hooks.json`) and **Grok**
  (`${GROK_HOME:-~/.grok}/hooks/*.json`) do **not** inject hooks at launch (`farhelm_args` returns
  `FarhelmArgs::none()`): they rely on user-installed manual hooks calling
  `farhelm internal hook --vendor <antigravity|grok>`. Tests must never modify the user's live home config (`AGENTS.md`,
  "The live install is off-limits").
- For Antigravity, `crates/farhelm-supervisor/src/service/core/vendor/positional.rs:189-203` rejects an unverified
  `SessionStart` and ignores an unverified `Stop`, so observing `snapshot.captured_conversation.is_some()`,
  `snapshot.restart_offer == RestartOffer::Resume`, and an `acked` entry in `hook-log/<session-id>.log` proves
  `antigravity::verifies_main_conversation` accepted the real transcript header.
- `scripts/agent_screen_host.py:57-73` already maintains `OUTER_SESSION_MARKERS` for stripping enclosing agent-session
  markers (`CLAUDECODE`, etc.). Rust tests must not call `unsafe` `std::env::remove_var`.
- `scripts/record-test-run.py:1165-1171` strips every `FARHELM_*` environment variable unless explicitly passed with
  `--keep-farhelm-env <NAME>`.
- Root `AGENTS.md` is currently 28,410 bytes against the hard 28,672-byte (28 KiB) limit enforced by
  `scripts/check-agents-md.py`.

### Acceptance criteria

1. **All 10 supported agent harnesses covered, exhaustively.** Every non-shell `LaunchHarness` variant (`Claude`,
   `Codex`, `Antigravity`, `Goose`, `Grok`, `Muse`, `Pi`, `Omp`, `Cursor`, `OpenCode`) has an opt-in `#[ignore]`d drift
   test in the Rust e2e suite (backed by an exhaustive match over `LaunchHarness` with
   `#[warn(clippy::wildcard_enum_match_arm)]` that explicitly excludes `Bash | Zsh`, so a future 11th agent harness is a
   compile error until added):
   - **8 tracked harnesses** (Claude, Codex, Antigravity, Goose, Grok, Muse, Pi, OMP): compile real production launch
     via `farhelm_helm::compile_agent_launch` (using a cheap/fast model where supported) → launch in a real supervisor
     and terminal host → clear trust/startup → send a tiny prompt ("reply with OK only") → verify the supervisor
     accepted the real conversation report (`snapshot.captured_conversation.is_some()`,
     `snapshot.restart_offer == RestartOffer::Resume`, and an `acked` entry in `hook-log/<session-id>.log` — for
     Antigravity, this proves `verifies_main_conversation` accepted the live transcript) → Stop and Restart through the
     real supervisor client → verify the resumed agent starts cleanly and stays alive without an immediate flag/resume
     error. Keep the existing Claude `/clear` and Codex `/new` tests intact.
   - **2 launch-only harnesses** (Cursor, OpenCode): compile real launch via `farhelm_helm::compile_agent_launch` →
     launch in a real supervisor and terminal host → verify the CLI accepts Farhelm's flags and reaches its interactive
     screen (or trust prompt) without an immediate flag-error exit.
2. **Usable three-way verdicts on any host (passed / could not be run / failed).**
   - Running the suite on a host where only some harnesses are installed, authenticated, or configured with manual hooks
     still runs every remaining harness and gives signal for all of them.
   - Clearly classify and report every selected harness into one of three distinct outcomes:
     - **PASSED** — ran end-to-end and passed every assertion.
     - **COULD NOT RUN** (with concrete reason: binary not installed, missing manual hook in user config for Antigravity
       or Grok, missing API key / login screen / auth error before a turn completes) — verdict unknown, never disguised
       as a pass.
     - **FAILED** — ran and broke (CLI rejected Farhelm's flags, hook failed/ignored, report rejected, Restart failed,
       timeout after auth succeeded), with diagnostic detail and the retained run path.
   - Never mutate the user's live Farhelm install or live hook configuration files (`~/.gemini/config/hooks.json`,
     `~/.grok/hooks/*.json`, etc.); if a manual hook is absent, classify as **COULD NOT RUN** pointing to the agent's
     setup guide.
3. **Thin, ergonomic runner script with recorder integration.** Add `scripts/run-harness-drift.py` (stdlib Python) and
   unit tests `scripts/test-run-harness-drift.py` (mocked subprocess/verdicts, zero vendor turns):
   - Flags: `--harness <name>` (repeatable; default runs all discovered drift harnesses) and `--list`.
   - Builds the e2e test binary once up front (`cargo test --no-run -p farhelm --test e2e`) and discovers the drift
     tests dynamically via `-- --ignored --list`, so Python does not maintain a duplicate hardcoded harness table.
   - Imports `OUTER_SESSION_MARKERS` from `scripts/agent_screen_host.py` and strips those markers plus `TMUX` /
     `TMUX_PANE` in the child environment rather than mutating env in Rust.
   - Runs each harness test through `scripts/record-test-run.py` with `--keep-farhelm-env FARHELM_DRIFT_VERDICT_FILE` so
     every run retains standard test evidence and the structured verdict JSON file.
   - Continues across all selected harnesses regardless of individual `could_not_run` or `failed` outcomes, prints a
     clear summary table grouping PASSED, COULD NOT RUN, and FAILED, and exits non-zero if any harness FAILED or if an
     explicitly requested `--harness` could not run (exits 0 on an all-harness run when every runnable harness passed,
     with COULD NOT RUN items clearly displayed).
   - If a drift test is run directly via `cargo test` without `FARHELM_DRIFT_VERDICT_FILE`, a missing binary/auth/hook
     fails loudly rather than silently returning `Ok`.
4. **Never automatic; wired into both requested manual flows.**
   - Never runs in default `cargo nextest`, deflake sweeps, CI, or release gates.
   - Add `docs/harness-drift-tests.md` documenting how to run the suite, what it checks, credentials/spend, manual-hook
     prerequisites for Antigravity and Grok, and the three-way verdicts.
   - Update `.agents/validation.md` with the opt-in runner and unit test (`scripts/test-run-harness-drift.py`), clearly
     marking the live runner opt-in/manual only.
   - Update `.agents/run-all-tests.md` to include triggering the real harness drift suite manually (explicitly noting
     its three-way verdicts and that it spends real vendor turns) and add `scripts/test-run-harness-drift.py` to its
     static/tooling checks.
   - Update root `AGENTS.md` Flows with a terse entry for `"run real harness tests"` pointing to
     `docs/harness-drift-tests.md`, keeping `AGENTS.md` strictly under 28 KiB and passing
     `python3 scripts/check-agents-md.py`.
5. **TODO.md updated.** In the final PR, remove both covered TODO.md entries (`Harness behavior drift tests.` and
   `Test that a real Antigravity still gets its conversation reports accepted.`) and add a new entry in the "Near term"
   bucket to extend the real-agent harness drift tests beyond the basic checks (for example: conversation rotation like
   `/clear`/`/new` across more harnesses, subagent-report rejection, permission/question prompts, and model/effort flag
   coverage).
6. **Live verification on this machine.** Run `scripts/run-harness-drift.py` live during execution against the harnesses
   installed and authenticated on the executing machine, verify that the authenticated harnesses pass (including
   Antigravity if its login and manual hook are present) and unconfigured ones report cleanly as `COULD NOT RUN`, and
   report the full per-harness summary (with recorder run IDs) in the plan report.

## Decisions already made

Settled with the maintainer on the 2026-11-20 planning page:

- **Folded scope:** Fold "Test that a real Antigravity still gets its conversation reports accepted" into this plan.
- **Harnesses:** All 10 supported agent harnesses (full basic lifecycle for the 8 tracked harnesses, launch-flags smoke
  for Cursor and OpenCode; keep existing Claude `/clear` and Codex `/new` tests).
- **Resilience and reporting:** Usable on any host where only a subset of harnesses are installed and authenticated;
  remaining harnesses still run and report three distinct states: **passed**, **could not be run** (unknown verdict),
  and **failed**.
- **Suite shape:** Planner's call — extend the existing `#[ignore]`d Rust e2e suite (reusing its real supervisor +
  terminal host harness) paired with a thin Python runner (`scripts/run-harness-drift.py`) that provides clean three-way
  verdict reporting without fighting Rust test tooling.
- **Triggering:** Never automatic on changes or in CI. Wire into the `"run all tests"` flow (`.agents/run-all-tests.md`)
  and add a `"run real harness tests"` trigger in `AGENTS.md` Flows.
- **Follow-up TODO:** Add a Near term TODO.md entry to extend the drift tests beyond these basic checks later.
- **Review gate:** A single fresh-context `gpt-6.2-sol` agent at `high` effort with the general review charter.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** The plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** The two TODO entries and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "The live install is off-limits" (never touch live Farhelm or user hook configs), "Code rules"
  (exhaustive per-harness matches), "Tests and validation" (update `.agents/validation.md` and
  `.agents/run-all-tests.md` when adding test scripts; run `python3 scripts/check-agents-md.py` when changing
  instruction files; keep `AGENTS.md` under 28 KiB).
- `.agents/real-agent-tui.md` (read before running real agents in terminals).
- `.agents/test-authoring.md` for every test added or changed, `.agents/test-evidence.md` (recording runs), and
  `scripts/check-test-sleeps.py` when Rust tests change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments" (`test:` needs no fragment unless user-facing).

**Planner proposals** (verified against the code and planning review):

- **Reuse `farhelm_helm::compile_agent_launch`** (already `pub` in `crates/farhelm-helm/src/lib.rs:188`) rather than
  exporting `launches::compile` or hand-assembling flags.
- **Generalize acceptance checks instead of duplicating vendor locator parsers.** Wait on
  `snapshot.captured_conversation.is_some() && snapshot.restart_offer == RestartOffer::Resume` plus an `acked` line in
  `hook-log/<session-id>.log`, then drive real `Stop` + `Restart` through the supervisor client. This works uniformly
  across `claude`, `codex`, `antigravity`, `goose`, `grok`, `muse`, `pi`, and `omp` without extending
  `reported_runtime_identity`/`resume_identity` for every JSON locator format.
- **Explicit manual-hook preflight for Antigravity and Grok.** Before spawning `agy` or `grok`, inspect
  `~/.gemini/config/hooks.json` or `${GROK_HOME:-~/.grok}/hooks/*.json` read-only for `farhelm internal hook --vendor`
  and record `could_not_run` if absent, avoiding false timeout failures.
- **Strip `OUTER_SESSION_MARKERS` in Python (`scripts/run-harness-drift.py`),** importing the constant from
  `scripts/agent_screen_host.py`, so Rust tests never mutate process environment.
- **Discover drift tests dynamically via `-- --ignored --list`** after one `cargo test --no-run -p farhelm --test e2e`
  build, using a fixed test-name prefix (e.g. `real_agent_capture::drift_<harness>`) so Python never duplicates the Rust
  harness enum.

## Outline

Two PRs, in this order:

1. `plan/harness-drift-tests/01-real-harness-e2e` (`test:`): extend `crates/farhelm/tests/e2e/real_agent_capture.rs` (or
   a shared helper module alongside it) with an exhaustive `LaunchHarness` drift test per non-shell agent harness using
   `farhelm_helm::compile_agent_launch`, real supervisor + terminal host, manual-hook preflight for Antigravity and
   Grok, Stop + Restart verification for the 8 tracked harnesses, launch smoke for Cursor and OpenCode, and
   `FARHELM_DRIFT_VERDICT_FILE` structured verdict output (still failing loudly when run without the env var). Keep
   existing Claude `/clear` and Codex `/new` tests passing.
2. `plan/harness-drift-tests/02-drift-runner-and-flows` (`test:`): add `scripts/run-harness-drift.py` (one upfront
   `--no-run` build, dynamic test discovery, `OUTER_SESSION_MARKERS` stripping,
   `scripts/record-test-run.py --keep-farhelm-env FARHELM_DRIFT_VERDICT_FILE`, three-way PASSED / COULD NOT RUN / FAILED
   table and exit code), add `scripts/test-run-harness-drift.py` unit tests, add `docs/harness-drift-tests.md`, update
   `.agents/validation.md`, `.agents/run-all-tests.md`, and root `AGENTS.md` Flows (`"run real harness tests"`, under 28
   KiB, verified with `check-agents-md.py`), and update `TODO.md` (remove both covered entries and add the Near term
   extension follow-up).

## Fallbacks

- If an installed agent CLI has a real upstream bug or authentication quirk that prevents a clean turn on this host
  during execution: if it is a missing login, expired token, or missing manual hook in `$HOME`, confirm the runner
  classifies it as `COULD NOT RUN` with an accurate reason and report it in the plan report (do not block on missing
  third-party credentials). If an authenticated harness genuinely fails because Farhelm's own production integration
  drifted against the installed CLI, keep the honest failing test or fix only if strictly a bug in the new test driver;
  do not silently weaken assertions to hide real upstream drift — record the real drift clearly in the report (and block
  only if you cannot tell whether the test driver itself is right).
- If `AGENTS.md` headroom (262 bytes at planning time) is tight even with a terse 2-line Flows entry, tighten redundant
  phrasing in `AGENTS.md` without changing any rule and verify with `python3 scripts/check-agents-md.py`.
- Decisions to log: the exact cheap/fast model chosen per harness for drift turns, the auth/login-prompt detection
  markers per harness, the exact verdict JSON schema, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-harness-drift-tests-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-harness-drift-tests-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. Run
`python3 scripts/check-agents-md.py` whenever changing `AGENTS.md` or `.agents/` files.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/harness-drift-tests/01-real-harness-e2e`
  and `plan/harness-drift-tests/02-drift-runner-and-flows`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves or extends the fragment its type calls for, describing in user-facing words
  what was built. Where what got built departs from what this file describes, the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes both covered TODO.md entries and adds the Near term follow-up TODO entry.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing both covered TODO.md
entries and adding the Near term follow-up TODO entry. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

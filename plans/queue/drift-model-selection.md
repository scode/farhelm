# Drift tests: categories framework and model selection across supporting agents

Written against main at 423770e10 on 2026-11-24. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Drift test feature coverage"). Verbatim:

> **Drift tests: model selection across all supporting agents.** Add live model-flag coverage for Cursor (`--model auto`
> and catalog models) and OpenCode (`--model opencode/<id>`), cover Goose, Pi, and OMP with both
> `--provider openrouter --model <id>` and no explicit provider, validate current catalog model IDs across Claude,
> Codex, Antigravity, and Muse (rather than only `haiku` or one model), and verify each CLI actually selects the
> requested model rather than silently ignoring it.

The maintainer's general direction across this batch of drift-test plans:

> the general idea here is: add the test category to our drift tests, plug in each harness. for each harness, make a
> reasonable effort to get it plugged in, and fix issues where it's straight forrward. if a harness cannot be plugged in
> easily, or it plugs in but there are bugs to fix that are not straight forward fixes, then create near term todo item
> to fix those issues but do not scope creep this plan to fix everything.

(The maintainer then settled that deferred TODO entries go in `## Plan follow-ups`, matching `plans/AGENTS.md`.)

This plan is also the prerequisite for the seven parallel feature-coverage drift plans queued behind it
(`(after drift-model-selection.md)`). In addition to implementing the `model_selection` drift category, its first PR
adds `--category` support to `scripts/run-harness-drift.py`, splits
`crates/farhelm/tests/e2e/real_agent_capture/drift.rs` into a modular `drift/` directory with shared launch/teardown
scaffolding, and exposes the two read-only `farhelm-helm` e2e helpers (`launcher_catalog` and `compile_named_start`)
that this plan and the parallel plans behind it share.

### Acceptance criteria

1. **Drift runner category support and modular Rust drift layout (PR 1):**
   - Split `crates/farhelm/tests/e2e/real_agent_capture/drift.rs` into
     `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs` (keeping `profile()`, `Terminal`, `Harness`,
     `TurnExpectation`, `wait_for_turn`, `private_config`, shared `mod tests`, plus extracted launch/teardown
     scaffolding, generalized case-registration macro, `TrustPolicy` on `Terminal::ready`, and
     `(resume_template, resume_argv)` conversation-selector extraction) and `drift/base.rs` (the existing 10 `base`
     cases).
   - Ignored drift tests follow `real_agent_capture::drift::<category>::drift_<harness>` across all categories (`base`
     cases move to `real_agent_capture::drift::base::drift_<harness>`).
   - `scripts/run-harness-drift.py` discovers `(category, harness)` pairs via
     `real_agent_capture::drift::([a-z][a-z0-9_]*)::drift_([a-z][a-z0-9_-]*): test`:
     - Adds repeatable `--category <name>` accepting category names or `all`.
     - Bare `scripts/run-harness-drift.py` (no `--category`) runs `base` only.
     - Combines `--category` with `--harness`; upgrades each unique selected harness once (never once per category);
       keys `results` by `(category, harness)` while keeping `upgrades` keyed by harness; keeps
       `explicit = bool(options.harness)` so an unselected unavailable harness in `--category <name>` does not fail the
       run while an explicitly named `--harness` does.
     - Rejects `--category` alongside `--upgrade-only` via `parser.error(...)` (preserving `--upgrade-only`'s no-build
       contract), and rejects an empty `(category, harness)` intersection via `parser.error(...)` naming the available
       harnesses in the selected categories.
     - `--list` prints discovered `category/harness` pairs filtered by any `--category`/`--harness`, and `summarize()`
       includes the category column.
   - `scripts/test-run-harness-drift.py` updates `DRIFT_SOURCE` for the split: verifies every `RECIPES` binary appears
     in `drift/mod.rs`, `drift/base.rs` registers exactly `sorted(RECIPES)`, and every `drift/*.rs` category file
     registers only harnesses in `RECIPES`. Adds unit tests for category discovery, default `base`, `--category all`,
     repeated `--category`, deduplicated upgrades, `(category, harness)` verdict keying, `--upgrade-only` + `--category`
     refusal, and empty intersection refusal.
   - Expose read-only `pub fn launcher_catalog() -> Vec<farhelm_proto::launcher::LaunchCatalogModel>` and
     `pub fn compile_named_start(launch: &farhelm_proto::SessionLaunch, value: String) -> Result<farhelm_proto::NamedConversationStart, String>`
     in `crates/farhelm-helm/src/lib.rs` (alongside `compile_agent_launch`), delegating directly to
     `launches::launcher_catalog` and `launches::named_start`.
   - Update `scripts/harness_upgrades.py` docstring, `docs/harness-drift-tests.md`, `.agents/validation.md`, and
     `.agents/run-all-tests.md` for the `--category` runner and split paths, and pass
     `python3 scripts/check-agents-md.py`.
2. **Model selection drift category across all supporting agents (PR 2):**
   - Add `crates/farhelm/tests/e2e/real_agent_capture/drift/model_selection.rs` and declare `mod model_selection;` in
     `drift/mod.rs`.
   - Register ignored `drift_<harness>` cases for every built-in harness where `LaunchHarness::offers_model()` is true
     (9 harnesses: Claude, Codex, Antigravity, Muse, Goose, Pi, OMP, Cursor, OpenCode; Grok has no model choice), pinned
     with a compile-time or unit guard against `LaunchHarness::BUILT_INS`.
   - Exercise all `latest: true` catalog models returned by `farhelm_helm::launcher_catalog()` for each harness:
     - Cursor: `--model auto` and every other `latest` Cursor catalog model (e.g. `composer-2.5`).
     - OpenCode: every `latest` `opencode/<id>` catalog model.
     - Goose, Pi, OMP: every `latest` `--provider openrouter --model <id>` catalog model (gated cleanly on
       `OPENROUTER_API_KEY` -> `CouldNotRun` if absent so missing OpenRouter credentials are not misreported) AND a
       launch with `model: None` (no explicit `--provider` or `--model`).
     - Claude, Codex, Antigravity, Muse: every `latest` catalog model (e.g. `claude-fable-5`, all 4 Codex models, all 4
       Antigravity models, `muse-spark-1.3-contributor`), not just `haiku` or one model.
   - Verify each CLI accepts Farhelm's compiled launch flags without refusal and actually selects the requested model
     (preferring screen/header/footer verification on startup so multi-model harnesses stay fast and fit inside the 720s
     per-case budget; only spending a short live turn if startup display cannot prove selection, or deferring if a
     harness needs non-trivial work).
   - Run the new `model_selection` category live with
     `python3 -B scripts/run-harness-drift.py --no-upgrade --category model_selection` against the installed CLIs on the
     executing host. Fix straightforward issues discovered during live runs; for any harness that cannot be plugged in
     cleanly or reveals non-trivial bugs, record a concrete entry under `## Plan follow-ups` in `TODO.md` rather than
     scope-creeping, and document the outcome honestly.
   - Remove the covered `TODO.md` entry (`Drift tests: model selection across all supporting agents.`) in the final PR.

## Decisions already made

### Maintainer decisions (binding)

- **Category framework & queue ordering:** This plan (`drift-model-selection`) introduces `--category` runner support
  and modularizes `drift.rs` into `drift/mod.rs` + per-category submodules; the other 7 drift plans run in parallel
  `(after drift-model-selection.md)`.
- **Default runner behavior:** Bare `scripts/run-harness-drift.py` runs `base` only; `--category <name>` (repeatable)
  and `--category all` select feature categories, combinable with `--harness <name>`.
- **Catalog breadth:** Test all `latest: true` catalog models on every supporting harness (plus no-provider default on
  Goose/Pi/OMP), using screen/header verification where the TUI shows the active model so startup-only checks stay fast.
- **Plugging in harnesses & deferral rule:** Make a reasonable effort to plug in each supporting harness and fix
  straightforward issues; if a harness cannot be plugged in easily or reveals non-trivial bugs, create a TODO entry in
  `## Plan follow-ups` and do not scope-creep the plan.
- **Live validation during execution:** Run the new category live with `--no-upgrade` against the currently installed
  CLIs on the executing host (do not run vendor upgraders during plan execution).
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- `AGENTS.md` ("Testability and tests"): Never mutate process environment variables in tests; use `SupervisorSeams`.
- `AGENTS.md` ("The live install is off-limits"): Never touch the maintainer's live Farhelm installation.
- `AGENTS.md` ("Tests and validation"): Changing `scripts/run-harness-drift.py` or `scripts/test-run-harness-drift.py`
  requires updating `.agents/validation.md` and `.agents/run-all-tests.md` in the same change and running
  `python3 scripts/check-agents-md.py`. Every test sleep needs a `// sleep-ok:` comment passing
  `scripts/check-test-sleeps.py`.
- `docs/harness-drift-tests.md`: Drift tests remain `#[ignore = "manual live vendor drift check"]` so ordinary nextest,
  CI, release gates, and deflake sweeps never launch real vendor CLIs.
- `.agents/validation.md`: `real_agent_capture::drift::tests` and `real_agent_capture::drift::private_config::tests`
  must remain discoverable under `test(/^real_agent_capture::drift::/)`.
- `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs`: `super::super::profile(*harness)` requires
  `profile()` to remain in `drift/mod.rs`.

### Planner-chosen implementation shape

- **Shared `drift/mod.rs` seams for all 8 plans:** Extract into `drift/mod.rs` during PR 1 only the concrete helpers
  that this plan and the 7 parallel plans behind it all use:
  1. Launch/teardown helper (`DriftEnv` / `with_drift_supervisor` + `update_override_prefix`) owning `which_binary` ->
     `CouldNotRun`, `SLOTS`, drop-ordered `config`/`work`/`state` tempdirs, optional `PrivateConfig`, `SupervisorSeams`,
     `serving_supervisor`, and guaranteed cleanup (`delete_session`, `accepting.stop()`, `private.finish()` even on
     error).
  2. Generalized case-registration macro (`drift_cases!` accepting a case handler function) so every category registers
     `drift_<harness>` with uniform `VERDICT_FILE` reporting.
  3. `TrustPolicy::{AutoAnswer, StopAtPrompt, FailOnPrompt}` on `Terminal::ready` so `drift-workspace-trust` can fail
     immediately on an unexpected trust prompt without duplicating `ready()`.
  4. A small helper reading the captured conversation selector from `(resume_template, resume_argv)` at
     `CONVERSATION_PLACEHOLDER`'s index so `drift-initial-prompt-launch` and `drift-named-conversation-starts` never
     duplicate vendor JSON locator parsing.
- **Shared `farhelm-helm` e2e exports in PR 1:** Export both `pub fn launcher_catalog()` and
  `pub fn compile_named_start()` from `crates/farhelm-helm/src/lib.rs` in PR 1 so Plans 2, 4, and 5 never touch
  `farhelm-helm/src/lib.rs` concurrently.
- **Startup-first model verification:** Prefer reading the active model name from the TUI's rendered
  header/banner/footer (or startup output) so 7–8 models on Goose/Pi/OMP/OpenCode finish comfortably within the 720s
  recorder timeout. If a vendor CLI does not display the model until a turn or hides it behind an alias, document the
  observed screen behavior and either check with one minimal turn (for small-catalog harnesses) or defer with a
  `## Plan follow-ups` entry if it would require heavy or fragile workarounds.

## Outline

### Existing mechanisms reused

- `scripts/run-harness-drift.py`, `scripts/harness_upgrades.py`, `scripts/test-run-harness-drift.py`.
- `crates/farhelm/tests/e2e/real_agent_capture/drift.rs` and `drift/private_config.rs`.
- `crates/farhelm-helm/src/lib.rs` (`compile_agent_launch`) and `crates/farhelm-helm/src/launches.rs`
  (`launcher_catalog`, `named_start`, `CATALOG`).
- `crates/farhelm-proto/src/launch.rs` (`LaunchHarness::BUILT_INS`, `offers_model`).

### PR stack

1. **`plan/drift-model-selection/01-drift-categories-and-module-split`** — Add category support to the drift runner and
   modularize the Rust drift suite:
   - Move `crates/farhelm/tests/e2e/real_agent_capture/drift.rs` to `drift/mod.rs` + `drift/base.rs`, extracting the
     shared launch/teardown helper, generalized `drift_cases!` macro, `Terminal::ready` `TrustPolicy`, and
     `(resume_template, resume_argv)` selector extractor while keeping `profile()`, `private_config`, and `mod tests` in
     `drift/mod.rs`.
   - Expose `pub fn launcher_catalog()` and `pub fn compile_named_start(...)` in `crates/farhelm-helm/src/lib.rs`.
   - Update `scripts/run-harness-drift.py` for `--category`, default `base`, `--category all`, deduplicated upgrades,
     `(category, harness)` result keys, `--upgrade-only` + `--category` refusal, empty intersection refusal, and
     category-aware `--list` and summary table.
   - Update `scripts/test-run-harness-drift.py` (fixing `DRIFT_SOURCE` and verifying all `drift/*.rs` harnesses against
     `RECIPES`), `scripts/harness_upgrades.py` docstring, `docs/harness-drift-tests.md`, `.agents/validation.md`, and
     `.agents/run-all-tests.md`.
2. **`plan/drift-model-selection/02-model-selection-drift-cases`** — Add `model_selection` drift category across all 9
   supporting harnesses:
   - Add `crates/farhelm/tests/e2e/real_agent_capture/drift/model_selection.rs` and wire `mod model_selection;` in
     `drift/mod.rs`.
   - Implement per-harness model verification using `farhelm_helm::launcher_catalog()` (all `latest: true` models per
     harness, Cursor `--model auto` + catalog models, OpenCode `opencode/<id>`, Goose/Pi/OMP OpenRouter models gated on
     `OPENROUTER_API_KEY` plus `model: None` default, Claude/Codex/Antigravity/Muse current catalog models).
   - Run `python3 -B scripts/run-harness-drift.py --no-upgrade --category model_selection` live; fix straightforward
     issues (adding a changelog fragment if any user-facing bug fix lands in production code); defer hard-to-plug
     harnesses or non-trivial bugs into `TODO.md` under `## Plan follow-ups`; remove the covered `TODO.md` entry.

## Fallbacks

- If an agent CLI is not installed or not logged in on the executing host (`COULD NOT RUN`), keep its case wired
  cleanly, record the exact `COULD NOT RUN` reason in the working log and final report, and do not mark it as a failure
  unless the harness's code itself is broken.
- If a harness cannot be plugged in easily (e.g., TUI does not display selected model and verifying selection would
  require invasive plumbing) or reveals a non-trivial bug in Farhelm's launch/catalog code, keep the case scoped cleanly
  (or leave that specific assertion/harness deferred), add a concrete entry under `## Plan follow-ups` in `TODO.md` with
  the observed CLI version, command line, and screen excerpt, and move on without scope-creeping.
- If a straightforward bug fix lands in production code (for example a catalog model ID typo or flag spelling fix),
  verify it with unit tests and add a `releasing/changelog.d/` fragment describing the user-facing fix.
- Scope-reassessment triggers (examples, not a blacklist): modifying wire protocol (`PROTOCOL_VERSION`), rewriting
  `harness_upgrades.py` upgrade mechanics, or redesigning `PrivateConfig` beyond what `drift/mod.rs` already needs.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-drift-model-selection-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-drift-model-selection-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- PR 1:
  - `python3 -B scripts/test-run-harness-drift.py` (through `record-test-run.py`).
  - `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'drift module unit tests' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`.
  - `python3 -B scripts/run-harness-drift.py --list` (verify `base/<harness>` discovery works without spending turns).
  - `cargo fmt --all -- --check`, `cargo clippy -p farhelm -p farhelm-helm --all-targets -- -D warnings`,
    `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, `dprint check` on changed Markdown, and
    `python3 scripts/check-agents-md.py`.
- PR 2:
  - `python3 -B scripts/test-run-harness-drift.py` (through `record-test-run.py`).
  - `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'drift module unit tests' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`.
  - `python3 -B scripts/run-harness-drift.py --list --category all`.
  - Live verification on currently installed CLIs:
    `python3 -B scripts/run-harness-drift.py --no-upgrade --category model_selection` (records per-case evidence
    automatically via `record-test-run.py`; record the resulting run IDs and verdicts in the working log and final
    report).
  - `cargo fmt --all -- --check`, `cargo clippy -p farhelm -p farhelm-helm --all-targets -- -D warnings`,
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
- The last PR of the stack removes the TODO.md entry this plan covers ("Drift tests: model selection across all
  supporting agents."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred harness issues added to
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

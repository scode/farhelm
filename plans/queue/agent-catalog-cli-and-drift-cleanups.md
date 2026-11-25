# Agent catalog, CLI, and drift cleanups

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says, and
where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This merged plan groups five small `TODO.md` entries from the "Immediate" bucket (plus one moot "Plan follow-ups" entry
superseded by GPT-5.6 removal), built as a clean linear stack of small PRs. Verbatim entries:

> **Remove GPT-5.6 models from the catalog.** OpenAI is retiring the GPT-5.6 models (Codex 0.172.0 already interrupts
> `gpt-5.6-terra` with a retirement prompt). Remove the `gpt-5.6-*` entries from Farhelm's model catalog across Codex,
> OpenRouter (Goose, Pi, OMP), and OpenCode, along with SPEC.md and tests.

> **Check Codex's GPT-5.6 Terra retirement path.** With `-m gpt-5.6-terra`, Codex 0.172.0 showed “Try new model” and
> “Use existing model” before it showed the composer. Reproduce with
> `python3 -B scripts/run-harness-drift.py --no-upgrade --category model_selection --harness codex`. Check the resulting
> model in an isolated CLI configuration if Farhelm still offers GPT-5.6 Terra. The live drift case leaves the account's
> choice alone.

> **Allow Goose named launches to capture and offer plain Restart.** When reopening a Goose conversation that Farhelm
> originally created, Goose runs its persisted `farhelm-reporter` MCP server, so Farhelm captures the destination
> conversation and offers plain Restart. `SPEC.md` and the UI copy currently state unconditionally that Goose never
> captures named conversations and Restart will be unavailable (lumping Goose with Cursor and OpenCode), which makes the
> `named_conversation` drift test fail
> (`python3 -B scripts/run-harness-drift.py --no-upgrade --category named_conversation --harness goose`). Update
> `SPEC.md`, the launcher/UI note, and the drift test so Restart is allowed and expected to work when a named Goose
> conversation carries Farhelm's reporter, while noting that Goose conversations created outside Farhelm are not
> captured. Check both named Create and named Restart.

> **Accept leading-hyphen custom `--effort` values as a separate argument.** `farhelm agent create --effort -fast` (with
> a space) is rejected by clap as an unknown flag even though custom agents allow `[a-z0-9_-]` effort values and
> `--model` already sets `allow_hyphen_values = true` (`crates/farhelm/src/main.rs`). Add `allow_hyphen_values = true`
> to `--effort` (matching `--model` and the `SPEC_impl.md` free-text option rule), add a parse test, and update
> `SPEC_impl.md` to drop the `--effort=-fast` workaround.

> **Deduplicate OpenCode's conversation lookup in the drift tests.** `initial_prompt` and `named_conversation` each look
> up the test conversation in OpenCode's session list by folder path, and the two copies disagree on symlink resolution
> and verdict (`COULD NOT RUN` vs `FAILED`). Share one helper so both categories behave the same way.

> **Derive `instructions_delivery` drift coverage from agent capabilities.** The `instructions_delivery` drift category
> hardcodes its six agent names instead of deriving them from agent capabilities like the other categories, so a future
> agent that gains the `$farhelm` hint would not fail coverage checks until added by hand.

### Acceptance criteria

1. **Remove GPT-5.6 models from the catalog (PR 1):**
   - Remove all five `gpt-5.6-terra` rows from `CATALOG` in `crates/farhelm-helm/src/launches.rs` (`gpt-5.6-terra` for
     Codex, `openai/gpt-5.6-terra` for Goose/Pi/OMP, `opencode/gpt-5.6-terra` for OpenCode) and remove `"terra"` from
     `CATALOG_FAMILIES` in `crates/farhelm-proto/src/launcher.rs` (keeping
     `catalog_families_match_the_launchers_reserved_words` passing).
   - Update `SPEC.md` to remove `opencode/gpt-5.6-terra` and `openai/gpt-5.6-terra` from suggested models.
   - Update affected tests in `crates/farhelm-helm/src/launches.rs` (`incompatible_and_malformed_models_are_refused` and
     `explicit_choices_use_each_harnesses_documented_flags`), `crates/farhelm/tests/e2e/github_checkouts.rs`, and
     optionally the serde fixture `"gpt-5.6-terra"` in `crates/farhelm-proto/src/launch.rs`
     (`structured_selection_rejects_unknown_fields`) to use a current Codex model.
   - Do **not** touch frozen fixtures (`crates/farhelm-supervisor/tests/fixtures/screens/`,
     `crates/farhelm-supervisor/tests/fixtures/status-lines/`), `CHANGELOG.md`, or `TRIAGE_OUTCOMES_ARCHIVE.md`. Leave
     `crates/gui/farhelm-gui/src/goldens/templates.rs` untouched if editing its mock catalog would force unnecessary
     golden PNG regeneration; log as a `DECISION`. Keep the general `codex_retirement_prompt` detector in
     `crates/farhelm/tests/e2e/real_agent_capture/drift/model_selection.rs`.
   - Confirm existing stored sessions, Clones, and explicit-harness templates referencing `gpt-5.6-terra` still work as
     custom models per `SPEC.md`.
   - Remove both covered GPT-5.6 `TODO.md` entries (_Remove GPT-5.6 models from the catalog._ and _Check Codex's GPT-5.6
     Terra retirement path._) in this PR.
2. **Allow Goose named launches to capture and offer plain Restart (PR 2):**
   - Replace `LaunchHarness::captures_named_conversation()` outright in `crates/farhelm-proto/src/launch.rs` with an
     exhaustive 3-way method `named_conversation_capture(self) -> NamedConversationCapture` (`Always`,
     `ExistingReporter`, `Never`, with `#[warn(clippy::wildcard_enum_match_arm)]` — no `bool` wrapper beside it), where
     `Goose` is `ExistingReporter`, `Cursor | OpenCode | Bash | Zsh` are `Never`, and the rest are `Always`.
   - Update all four call sites cleanly:
     - `crates/farhelm-supervisor/src/service/core.rs`: **preserve transactional capture clearing on named Goose
       Restart** (`clear_capture` MUST remain `true` for `ExistingReporter` and `Never`, restoring old capture on
       definitive pre-spawn failure and keeping
       `named_goose_restart_clears_live_capture_and_restores_it_on_definitive_failure` passing) so reopening an external
       Goose conversation never leaves the previous conversation ID stuck as the Restart target, while reopening a
       Farhelm-created Goose conversation lets its persisted `farhelm-reporter` report the destination and enable plain
       Restart.
     - `crates/gui/farhelm-gui/src/launcher/view/more_options.rs`: **drop the help line for Goose** — show
       `"Farhelm will not capture this conversation after a named start, so Restart will not be available."` only for
       `Never` harnesses (Cursor, OpenCode) and custom agents without reporting, **not** for Goose. Update
       `crates/gui/farhelm-gui/src/launcher/view_tests.rs`
       (`prompt_and_conversation_rows_follow_the_agents_capabilities`).
     - `crates/farhelm-helm/src/launches.rs`: update the `resume_argv()` `None` assertion (`Never`).
     - `crates/farhelm/tests/e2e/real_agent_capture/drift/named_conversation.rs`: dispatch directly on
       `NamedConversationCapture` (`Always`, `ExistingReporter`, `Never`) so Goose's drift case tests **both** named
       Create and named Restart on a Farhelm-created Goose conversation. On named Restart, assert immediately after
       spawn that `restarted.captured_conversation != seed.captured_conversation` (allowing either `None` before report
       or `Some(target)` if Goose's persisted MCP reporter fired immediately — do not call `ensure_uncaptured`), then
       wait for recall, `RestartOffer::Resume`, and `selector == target`.
   - Update `SPEC.md`, `SPEC_impl.md`, and `docs/harness-drift-tests.md` to document that named Goose launches capture
     and offer plain Restart when reopening a Goose conversation originally created by Farhelm (which carries Farhelm's
     persisted reporter), while external Goose conversations are not captured, and that named Goose Restart still clears
     the prior capture first.
   - Run the live Goose drift check if Goose is installed and signed in on the executing machine (or record honestly if
     unavailable, without breaking offline e2e tests).
   - Remove the covered Goose `TODO.md` entry in this PR.
3. **Accept leading-hyphen custom `--effort` values as a separate argument (PR 3):**
   - Add `allow_hyphen_values = true` to `LaunchFlags::effort` in `crates/farhelm/src/main.rs` (matching `--model`).
   - Add a CLI parse unit test verifying `farhelm agent create --effort -fast` (space-separated) parses into
     `LaunchEffortValue::Custom("-fast")`.
   - Update `SPEC_impl.md` where `--effort` currently documents the `--effort=-fast` workaround so `--effort` is grouped
     with `--model` and `--cwd` as a free-text option accepting leading hyphens as a separate argument.
   - Remove the covered `--effort` `TODO.md` entry in this PR.
4. **Deduplicate OpenCode's conversation lookup in the drift tests (PR 4):**
   - Move `vendor_lookup` from `crates/farhelm/tests/e2e/real_agent_capture/drift/named_conversation.rs` into
     `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs` and build one shared OpenCode workspace-session lookup
     helper on top of it (using `profile.binary`,
     `["session", "list", "--pure", "--format", "json", "--max-count", "32"]`, the 30s timeout and `refusal()` error
     handling, canonicalizing the workspace path and `session.directory` with raw-path fallback, returning
     `DriftError::Failed` on 0 or >1 matching sessions after a succeeded turn). That way Cursor's `create-chat` and both
     OpenCode callers share the same subprocess/timeout/`refusal()` plumbing.
   - Call that shared OpenCode helper from both `initial_prompt.rs` (leaving `Cursor` returning `Ok(None)` →
     `CouldNotRun`) and `named_conversation.rs`.
   - Move `opencode_only_resumes_a_session_in_the_test_workspace` to `drift/mod.rs` and add a symlinked-workspace unit
     test pinning canonicalization.
   - Remove the covered OpenCode lookup `TODO.md` entry in this PR.
5. **Derive `instructions_delivery` drift coverage from agent capabilities (PR 5):**
   - Expose a `#[cfg(feature = "test-seams")] pub fn delivers_instructions(kind: AgentKind) -> bool` helper in
     `crates/farhelm-supervisor/src/agent_kind/mod.rs` derived directly from `integration_for(kind)` by comparing
     `(on.args, on.env) != (off.args, off.env)` from `farhelm_args(LaunchPhase::Start, ...)` with
     `AgentInstructions::On` vs `AgentInstructions::Off` (note: Goose delivers instructions via `FarhelmArgs.env`, not
     `args`, so both `args` and `env` MUST be compared). Document it in `agent_kind/mod.rs`'s capability map.
   - Add a unit test in `crates/farhelm-supervisor/src/agent_kind/mod.rs` pinning the derived set across
     `AgentKind::ALL` (`Claude`, `Codex`, `Muse`, `Goose`, `Pi`, `Omp` true; the rest false).
   - Update `crates/farhelm/tests/e2e/real_agent_capture/drift/instructions_delivery.rs` to derive its expected harness
     list from `LaunchHarness::BUILT_INS.iter().copied().filter(|h| delivers_instructions(h.agent_kind()))` instead of
     hardcoding six agent names.
   - Remove the covered `instructions_delivery` `TODO.md` entry in this PR.

## Decisions already made

### Maintainer decisions (binding)

- **Grouping:** Group these five small agent/catalog/CLI/drift items into this one plan, with one small PR per item.
- **Goose launcher UI note:** Drop the "will not capture" help line for Goose under **More options → Session ID**
  altogether (do not replace it with a conditional note; keep the warning for Cursor, OpenCode, and non-reporting custom
  agents).
- **Review gate:** Single fresh-context agent with the general charter on `Opus 5.5` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- `AGENTS.md` & `crates/farhelm-supervisor/src/agent_kind/mod.rs`: Everything Farhelm does differently per agent harness
  is answered per harness in the named exhaustive places, never via `kind == X`, `matches!` over specific harnesses, or
  a `_` arm in shared code.
- `AGENTS.md`: Never edit frozen fixtures (`crates/farhelm-supervisor/tests/fixtures/screens/`), never mutate process
  environment variables in tests, never touch the live Farhelm install, annotate every test sleep with `// sleep-ok:`.

### Planner-chosen implementation shape

- **Replace `LaunchHarness::captures_named_conversation()` outright with 3-way
  `NamedConversationCapture::{Always, ExistingReporter, Never}`** rather than keeping a `bool` wrapper beside it, so the
  supervisor keeps `clear_capture = true` on named Goose Restart, the launcher drops the warning for Goose, and the
  drift test dispatches on all three cases directly.
- **Derive `delivers_instructions(kind)` in `farhelm-supervisor::agent_kind` (`#[cfg(feature = "test-seams")]`) directly
  from `integration_for(kind).farhelm_args(...)` on `(args, env)`** rather than adding a test-only boolean table to
  `farhelm-proto`, avoiding widening the wire crate and avoiding a second hand-maintained copy of which harnesses
  deliver `$farhelm`.

## Outline

### Existing mechanisms reused

- `crates/farhelm-helm/src/launches.rs` (`CATALOG`) and `crates/farhelm-proto/src/launcher.rs` (`CATALOG_FAMILIES`).
- `crates/farhelm-proto/src/launch.rs` (`LaunchHarness::captures_named_conversation`),
  `crates/farhelm-supervisor/src/service/core.rs` (`clear_capture`), `crates/farhelm-helm/src/launches.rs`
  (`resume_argv`), and `crates/gui/farhelm-gui/src/launcher/view/more_options.rs`.
- `crates/farhelm/src/main.rs` (`LaunchFlags`, `parse_effort`).
- `crates/farhelm/tests/e2e/real_agent_capture/drift/{mod,initial_prompt,named_conversation,instructions_delivery}.rs`
  (`vendor_lookup`).
- `crates/farhelm-supervisor/src/agent_kind/mod.rs` (`integration_for`, `AgentInstructions`,
  `#[cfg(feature = "test-seams")]`).

### PR stack

1. **`plan/agent-catalog-cli-and-drift-cleanups/01-remove-gpt-5-6-catalog`** — Remove `gpt-5.6-terra` and `"terra"` from
   catalog/families, update `SPEC.md` and affected tests, add changelog fragment, remove both GPT-5.6 `TODO.md` entries.
2. **`plan/agent-catalog-cli-and-drift-cleanups/02-allow-goose-named-restart`** — Replace `captures_named_conversation`
   with 3-way `NamedConversationCapture`, drop Goose's launcher help line while keeping `clear_capture = true` on named
   Goose Restart, update all four callers, `SPEC.md`, `SPEC_impl.md`, `docs/harness-drift-tests.md`, unit tests, and
   Goose's `named_conversation` drift test (both Create and Restart), add changelog fragment, remove the Goose `TODO.md`
   entry.
3. **`plan/agent-catalog-cli-and-drift-cleanups/03-cli-effort-allow-hyphens`** — Add `allow_hyphen_values = true` to
   `--effort`, add parse test, update `SPEC_impl.md`, add changelog fragment, remove the `--effort` `TODO.md` entry.
4. **`plan/agent-catalog-cli-and-drift-cleanups/04-dedup-opencode-drift-lookup`** — Move `vendor_lookup` and share
   canonicalized OpenCode workspace session lookup in `drift/mod.rs`, add symlink unit test, remove the OpenCode lookup
   `TODO.md` entry.
5. **`plan/agent-catalog-cli-and-drift-cleanups/05-derive-instructions-drift-coverage`** — Derive
   `delivers_instructions(kind)` under `test-seams` in `farhelm-supervisor::agent_kind` from `farhelm_args`
   `(args, env)`, pin with a unit test, update `instructions_delivery.rs`, remove the `instructions_delivery` `TODO.md`
   entry.

## Fallbacks

- If Goose is not installed or signed in on the executing machine, run all offline drift/unit/e2e tests and record the
  unrun live Goose drift check honestly in the report (do not block solely on vendor login unless a real bug appears).
- If `crates/gui/farhelm-gui/src/goldens/templates.rs` changes any golden PNG when touched, leave its mock model string
  alone and log as a `DECISION`.
- Scope-reassessment triggers (examples, not a blacklist): changing `PROTOCOL_VERSION` or database schemas, rewriting
  Goose's MCP reporter injection, or altering how custom-agent effort validation rules work.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-agent-catalog-cli-and-drift-cleanups-log.md` in the parent directory of the checkout you run in, derived
as that section says, and the watchdog's status file is
`farhelm-plan-agent-catalog-cli-and-drift-cleanups-watchdog.json` beside it. Resolve both to absolute paths before you
start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`, "Agent
scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- Targeted Rust tests (through `record-test-run.py` with pinned nextest on `PATH`):
  - `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E 'test(launches::) | test(launch::) | test(catalog) | test(agent_kind::) | test(named_goose_restart)'`
  - `cargo nextest run -p farhelm --bin farhelm --test agent_cli --test e2e -E 'test(/^real_agent_capture::drift::/) | test(custom_create_) | test(github_checkouts) | test(effort)'`
  - `cargo nextest run -p farhelm-gui --lib -E 'test(prompt_and_conversation_rows_follow_the_agents_capabilities)'`
  - Optional live verification if Goose is installed and authenticated:
    `python3 -B scripts/run-harness-drift.py --no-upgrade --category named_conversation --harness goose`
- Formatting, lints, and checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
  - `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`
  - `dprint check` on changed Markdown files
  - `python3 releasing/check-changelog.py format`
  - `python3 scripts/check-agents-md.py` (if headings or instruction files change)

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
- Each PR removes the `TODO.md` entry (or entries) it addresses, so all six covered entries are removed by the top of
  the stack, as root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred items added to `TODO.md` go under
  `## Plan follow-ups`.
- Commit messages, PR titles, and changelog fragments follow root `AGENTS.md`, "Conventional Commits" and "Changelog
  fragments". This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you (`plans/.agents/executing.md`, "Executing one plan", step 8); never edit that section.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `Opus 5.5` at `high` effort,
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

Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular whether `goldens/templates.rs` was left untouched, the exact
`NamedConversationCapture` naming, and any review finding you decided not to follow. The maintainer will ask for these
later.

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
DECISION, or it is a block. Things the maintainer should know and possible follow-ups go into the report as statements.

### Done criterion

The goal is achieved when every PR in the linear stack exists on GitHub as a draft PR and has passed its review gate,
and the latest `## Decisions` entry, if any, is satisfied. Delivering the report is your closing step per
`plans/.agents/executing.md`, "Executing one plan" (step 11), not part of the goal.

## Release note

No release note needed.

# Fix Muse Code 1.4.24 conversation reporting across hook environment scrubbing

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Evidence shows Muse Code conversation reporting is broken (1.4.24).** During the `harness-drift-tests` live run on
> Muse Code 1.4.24-R9131.1, Farhelm injected its hook flags and Muse replied to the prompt, but no conversation report
> arrived and Farhelm's hook helper never even wrote a diagnostic log line before teardown—indicating conversation
> reporting is broken on current Muse Code so Restart cannot resume Muse conversations. Find out why the callback did
> not fire and fix or adapt Farhelm's Muse integration.

Planning diagnosed the exact root cause offline against Muse Code 1.4.24-R9131.1 (`--provider echo`): Muse still fires
both `SessionStart` and `Stop` hooks from Farhelm's `-c hooks=...` override and delivers the expected JSON payload
(`session_id`, `hook_event_name`, `source`) on stdin, **but now sanitizes the environment passed to hook child
processes**, stripping `FARHELM_SESSION_ID`, `FARHELM_SUPERVISOR_SOCKET`, and `FARHELM_SESSION_TOKEN` (and
`-c managed_hooks_env_vars=[...]` does not pass them through to inline `-c hooks=...` handlers). Because
`farhelm internal hook` reads those three values exclusively from the environment, it saw none of them and exited
silently without writing a report or hook log line.

In product terms: pass non-secret `--session-id` and `--supervisor-socket` directly on Muse's per-launch injected hook
command (never passing the secret `FARHELM_SESSION_TOKEN` on argv), let `farhelm internal hook` accept explicit
per-launch `--session-id` + `--supervisor-socket` arguments without requiring `FARHELM_SESSION_TOKEN` in the
environment, verify end-to-end offline against real Muse Code 1.4.24 (`--provider echo`), and update specs/docs.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Muse hook injection (`crates/farhelm-supervisor/src/agent_kind/muse.rs`, `agent_kind/mod.rs`,
  `crates/farhelm-supervisor/src/service/core.rs`):**
  - `MuseIntegration::hook_argv` (`muse.rs:32-44`) builds `-c hooks={"SessionStart":[...],"Stop":[...]}` using
    `hook_command(hook_exe, instructions, ReportVendor::Muse)`.
  - `HookPolicy<'_>` (`agent_kind/mod.rs`) is constructed in **`crates/farhelm-supervisor/src/service/core.rs`**
    (`with_farhelm_args_using` at line 2967 and `legacy_hook_injection` at line 3141, called from
    `Supervisor::with_farhelm_args` at line 14409 inside `Supervisor::prepare_launch` at line 14580, where `session` and
    `Self::socket_path(&self.state_dir)` at line 14655 are already available).
  - Thread those non-secret per-launch values (`session_id` and `supervisor_socket`) into `HookPolicy<'_>` and pass
    `policy: &HookPolicy<'_>` into `AgentIntegration::hook_argv` (1-line updates in `ClaudeIntegration` and
    `CodexIntegration` to read `policy.instructions`) so `MuseIntegration` reuses `hook_tail_args` and
    `inject_hook_argv_tail` directly without duplicating opt-out, bare-`--`, refusal, or logging logic across both
    structured (`farhelm_args`) and legacy (`inject_hooks`) launches. If `supervisor_socket` is `Option<&'a str>` (e.g.,
    non-UTF-8 path in a unit test), handle `None` explicitly so it is logged as `HookLog::Skipped` rather than silently
    treated like a silent-hook harness.
  - **Security invariant:** NEVER pass `FARHELM_SESSION_TOKEN` on argv (visible to other local users via `ps`). The
    supervisor's hook admission (`service/core/vendor/positional.rs`) already verifies the hook process's ancestry sits
    inside the session's terminal and reads reports only from the supervisor's own `0700` state directory.
- **Hook helper (`crates/farhelm/src/main.rs:2094-2096`, `crates/farhelm/src/agent_client.rs:131-166`,
  `crates/farhelm/src/hook.rs:50-60`):**
  - Add optional `--session-id <ID>` and `--supervisor-socket <PATH>` CLI flags to `farhelm internal hook` as
    **independent `Option` flags** — do NOT use clap's `requires` attribute (which prints to stderr and exits 2,
    violating `hook.rs`'s silence/exit-0 contract).
  - Add a pure resolution method on `SessionEnv` in `crates/farhelm/src/agent_client.rs` (which already owns
    `SessionEnv::hook_log` and `SessionEnv::hook_credential`, keeping socket-parent / hook-log derivation in one place
    and unit-testable without mutating process env vars):
    - When BOTH `--session-id` and `--supervisor-socket` are passed explicitly (a per-launch Farhelm-injected command,
      never present when a user runs an agent outside Farhelm), derive the credential and hook log path directly without
      requiring `FARHELM_SESSION_TOKEN` in the environment (preferring explicit CLI flags over scrubbed/missing env
      vars).
    - When ONLY ONE flag is supplied with no env fallback, return no credential (exiting 0 silently without writing a
      hook log line, since there is no complete `(session_id, socket)` pair to derive a log path).
    - When BOTH flags are omitted, keep the existing environment-variable + `FARHELM_SESSION_TOKEN` requirement
      untouched (so persistent user-installed hooks for Grok/Antigravity and other vendors' env-based hooks keep their
      exact outside-Farhelm no-op safety).
  - Verify shell quoting in `muse.rs` handles spaces/quotes/special characters in `<socket>` and `<exe>` cleanly (just
    as `muse_hook_argv_roundtrips_json_and_shell_quoting` tests `<exe>` today).
- **Specs, docs, fixtures, and tests:**
  - Update `SPEC_impl.md:4435-4520`, `website/src/content/docs/docs/how-it-works/agent-hook-injection.md`, and code doc
    comments in `crates/farhelm/src/hook.rs:50-60` and `crates/farhelm/src/agent_client.rs:131-166`.
  - Update `crates/farhelm-fixtures/src/fake_agent.rs:1341-1356, 1383-1405` (`muse-hook-report`) to strip `FARHELM_*`
    from Muse's hook child `Command` (before `run_report_hook` sets `FARHELM_TEST_HOOK_BUDGET_MS`), and rebuild
    `farhelm-fixtures` before running `crates/farhelm/tests/e2e/hook_identity.rs` (`MUSE_FIXTURE`) so the e2e test
    proves capture and Restart survive env scrubbing.
  - Add unit tests in `crates/farhelm-supervisor/src/agent_kind/muse.rs`, `crates/farhelm/src/agent_client.rs`, and
    `crates/farhelm/src/hook.rs`.
  - **Zero-cost offline verification only (unattended):** Verify against installed Muse Code 1.4.24 offline via
    `MUSE_NO_AUTO_UPDATE=1 muse exec --provider echo` (and `scripts/agent-tui.py` with `--provider echo` if interactive
    TUI behavior needs checking). Do **not** run live `scripts/run-harness-drift.py` / `drift_muse` unattended — root
    `AGENTS.md` fences real harness drift tests as opt-in only because they spend vendor turns; note live `drift_muse`
    as a post-landing check for the maintainer in the delivery report.

### Acceptance criteria

1. `farhelm internal hook` accepts independent optional `--session-id` and `--supervisor-socket` flags (no clap
   `requires`, preserving exit-0 silence); when both are supplied, `SessionEnv` resolves the credential and hook log
   path even when all `FARHELM_*` environment variables have been stripped, without requiring `FARHELM_SESSION_TOKEN`.
   When only one is supplied with no env fallback, it exits 0 silently. When both are omitted, existing env-var + token
   rules remain unchanged.
2. `MuseIntegration` (reusing `hook_tail_args` and `inject_hook_argv_tail` via `&HookPolicy`) passes shell-quoted
   `--session-id` and `--supervisor-socket` on its per-launch `-c hooks=...` commands (`SessionStart` and `Stop`, Start
   and Resume), never puts `FARHELM_SESSION_TOKEN` on argv, and still honors `FARHELM_AGENT_HOOKS=none` and user
   `-c hooks=...` refusal.
3. Unit tests (`muse.rs`, `agent_client.rs`, `hook.rs`) and fixture e2e tests (`fake_agent.rs` scrubbing `FARHELM_*` +
   `hook_identity.rs`) prove Muse conversation capture and Restart work when the agent scrubs all `FARHELM_*`
   environment variables from hook child processes, and prove no secret token appears in Muse's argv.
4. Verified offline against real Muse Code 1.4.24 (`--provider echo`, zero vendor turns) showing Farhelm's hook helper
   receives the payload and writes the report + hook log line (leaving live vendor-turn `drift_muse` for the maintainer
   per `AGENTS.md`'s opt-in fence).
5. `SPEC_impl.md`, `agent-hook-injection.md`, and code doc comments in `hook.rs` / `agent_client.rs` are updated, and
   the TODO.md entry is removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Pass non-secret `--session-id` and `--supervisor-socket` directly on Muse's per-launch hook command (never passing the
  secret session token on argv).
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: verbatim text recorded under `## Release note` below.

**Binding repository constraints:**

- `AGENTS.md`, "Code rules": keep Muse-specific hook argv construction in `MuseIntegration` (`agent_kind/muse.rs`).
- `AGENTS.md`, "Flows" ("Real agent tests"): opt-in only, spends vendor turns — do NOT run live `drift_muse` unattended.
- `website/AGENTS.md` for `agent-hook-injection.md` (unattended run: no preview server).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Pass `policy: &HookPolicy<'_>` into `AgentIntegration::hook_argv` so `MuseIntegration` reuses `hook_tail_args` and
  `inject_hook_argv_tail` without duplicating opt-out/refusal/logging logic.
- Centralize CLI-flag + env credential/log resolution in a pure method on `SessionEnv` (`agent_client.rs`).
- Deliver in 1 cohesive PR (`fix: pass session identity args to Muse Code hooks`).

## Outline

1. `plan/fix-muse-conversation-reporting/01-muse-hook-cli-identity`: add independent optional `--session-id` and
   `--supervisor-socket` flags to `farhelm internal hook` (`crates/farhelm/src/main.rs`) resolved via a pure method on
   `SessionEnv` (`crates/farhelm/src/agent_client.rs`), thread non-secret session id and supervisor socket path into
   `HookPolicy` from `crates/farhelm-supervisor/src/service/core.rs` and pass `&HookPolicy` to
   `AgentIntegration::hook_argv` so `MuseIntegration` passes them shell-quoted on `SessionStart` and `Stop` (never
   passing `FARHELM_SESSION_TOKEN`), update `crates/farhelm-fixtures/src/fake_agent.rs` (`muse-hook-report`) to scrub
   `FARHELM_*` from hook child env and add unit + e2e tests (`muse.rs`, `agent_client.rs`, `hook.rs`,
   `hook_identity.rs`), verify offline against real Muse Code 1.4.24 (`--provider echo`), update `SPEC_impl.md`,
   `website/src/content/docs/docs/how-it-works/agent-hook-injection.md`, and doc comments in `hook.rs` /
   `agent_client.rs`, add a `kind: fixed` changelog fragment, and remove the TODO.md entry ("Evidence shows Muse Code
   conversation reporting is broken (1.4.24)"). `fix: pass session identity args to Muse Code hooks`.

## Fallbacks

- If offline testing against real Muse Code 1.4.24 reveals a second issue beyond env scrubbing (for example, a hook
  trust prompt in interactive TUI mode that `muse exec --provider echo` did not trigger), investigate it offline with
  `scripts/agent-tui.py` and `muse --provider echo` per `.agents/real-agent-tui.md`, run scope reassessment if a
  substantial new mechanism is required, and block if it requires a product decision (such as moving Muse to opt-in
  persistent hooks).

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-fix-muse-conversation-reporting-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-fix-muse-conversation-reporting-watchdog.json` beside
it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent
scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Rebuild `farhelm-fixtures` before running e2e tests, and run targeted tests in `farhelm-supervisor`
(`agent_kind::muse`), `farhelm` (`agent_client`, `hook`, `hook_identity`, `structured_launches`), zero-cost offline Muse
1.4.24 `--provider echo` verification (do NOT run live vendor-turn `drift_muse` unattended), website checks for edited
docs, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `dprint check` on changed Markdown, and
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to
every test you add or change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Evidence shows Muse Code conversation reporting
  is broken (1.4.24)"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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

Muse Code session tracking was not functioning due to environment variable scrubbing performed by Muse Code (likely a
recent change). This should now be fixed.

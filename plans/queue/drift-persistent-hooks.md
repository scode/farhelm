# Plan: Opt-in persistent hook installation drift tests for Grok and Antigravity

## Dependency

Depends on `plans/queue/drift-base-isolation-and-turns.md` landing on `main` first
(`after drift-base-isolation-and-turns.md`), so this plan reuses the shared `send_prompt` and `wait_for_reply` helpers
moved into `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs`.

## Goal

Add a dedicated `persistent_hooks` drift category
(`crates/farhelm/tests/e2e/real_agent_capture/drift/persistent_hooks.rs`) that exercises Farhelm's opt-in persistent
hook files for every agent kind where `AgentKind::offers_persistent_hooks()` is true (today Grok and Antigravity):
consented launch installs the exact planned hook files into an isolated config mirror and captures conversations inside
Farhelm, and running the real CLI outside Farhelm against that same installed mirror reaches its composer and completes
a short no-tools turn with every installed hook exiting cleanly and silently while the real user home remains untouched.

Original `TODO.md` entry (under `## Near term` → `### Drift test feature coverage`):

> **Drift tests: opt-in persistent hook installation for Grok and Antigravity.** Using isolated config directories so
> the real user home stays untouched, verify that the exact hook files Farhelm installs for Grok (`hooks/farhelm.json`)
> and Antigravity (`plugins/farhelm/{plugin.json,hooks.json}`) are discovered and invoked by the real CLIs inside
> Farhelm to capture conversations, and exit cleanly and silently when the CLIs run outside Farhelm.

### Acceptance criteria

- A new `persistent_hooks` drift category is discovered by `scripts/run-harness-drift.py` and covers every built-in
  harness where `harness.agent_kind().offers_persistent_hooks()` is true (`Grok`, `Antigravity`), pinned by a
  non-ignored unit guard test so future persistent-hook agents cannot skip coverage.
- Using `drift/private_config.rs`'s isolated mirror (never mutating the real `$HOME` or `$GROK_HOME` hook files), one
  shared `execute(harness)` driver runs two phases inside one `with_drift_supervisor` scope per harness:
  1. **Consented install and live capture inside Farhelm:** Farhelm installs the exact planned hook files into the
     mirror (paths supplied by `persistent_hook_paths_for_test`: `hooks/farhelm.json` for Grok and
     `plugins/farhelm/{plugin.json,hooks.json}` for Antigravity; `private.check_hook_setup(true)` passes), the real CLI
     discovers and invokes them on one short no-tools turn to capture a conversation (`wait_for_turn` +
     `hook_invisible`, no Restart needed since `base` already covers Restart), and real user hook files remain
     unchanged.
  2. **Live execution outside Farhelm against the same installed mirror:** Launch the real CLI as a plain command
     (`DriftEnv::with_plain_session`) that generically unsets every `FARHELM_*` environment variable, explicitly pins
     the mirror's redirect variable (`GROK_HOME` / `HOME`) after login-shell startup, and `exec`s the vendor CLI so
     `child_root()` is the vendor CLI itself. Before sending any keystroke, verify via `/proc/<child_root>/cmdline` and
     `/proc/<child_root>/environ` that the child is the vendor binary, carries **zero** `FARHELM_*` variables, and
     carries the mirror's redirect path. Then reach the composer, complete one short split-nonce no-tools turn cleanly
     (exercising `SessionStart`, `UserPromptSubmit`, and `Stop` for Grok, and `SessionStart` and `Stop` for
     Antigravity), verify `hook_invisible` (no leaked hook error, `farhelm internal hook` diagnostic, or protocol reply
     like Antigravity's `{}` on screen or live output), confirm the CLI stays live, and confirm real user hook files
     remain untouched.
- Shared code carries the whole driver and helper layer (reusing `send_prompt` / `wait_for_reply` from `drift/mod.rs`,
  and promoting `read_child_proc` and `hook_invisible` from `drift/capture_timing_hooks.rs` to `drift/mod.rs`); harness
  specifics stay in small exhaustive `#[warn(clippy::wildcard_enum_match_arm)]` functions.
- Non-ignored unit tests pin the exhaustive `offers_persistent_hooks()` guard, the `FARHELM_*`-absence + mirror-env
  `environ` parser, and the outside-Farhelm command builder.
- `docs/harness-drift-tests.md` documents the new `persistent_hooks` category.
- The last code PR removes this entry from `TODO.md`.

## Requirement sources

### Maintainer request and settled decisions

- **Dedicated `persistent_hooks` category:** Keep this as its own drift category rather than folding it into
  `capture_timing_hooks`.
- **Outside-Farhelm depth:** Reach the composer AND complete one short no-tools turn outside Farhelm so every installed
  hook event (`SessionStart`, `UserPromptSubmit`, `Stop` on Grok; `SessionStart`, `Stop` on Antigravity) fires without
  Farhelm credentials and proves clean, silent exit.
- **Maximize shared code:** Use shared code as much as possible and plug in harness-specific code only where required.
- **No line-count gates; complexity fallback:** Do not enforce artificial line-count gates. If any part of this plan
  turns disproportionately complicated during execution, stop sinking complexity into it, leave that piece out, and add
  a `## Plan follow-ups` entry in `TODO.md` (`plans/AGENTS.md`, "Follow-up TODO entries") explaining what got
  complicated and why.
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter.

### Binding repository constraints

- `AGENTS.md`, "Code rules" and `crates/farhelm-supervisor/src/agent_kind/mod.rs`: no `kind == X`, `matches!` over
  specific harnesses, or `_` arms over harnesses in shared code; use exhaustive per-harness functions with
  `#[warn(clippy::wildcard_enum_match_arm)]`.
- `AGENTS.md`, "The live install is off-limits": an outside-Farhelm run must never inherit live `FARHELM_*` variables
  (which could make the built hook report to the maintainer's live supervisor) and must never touch real `$HOME` /
  `$GROK_HOME` hook files or connect to an unrelated host daemon outside the mirror.
- `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs`: persistent hook paths always come from
  `farhelm_supervisor::agent_kind::persistent_hook_paths_for_test` (never duplicate path lists), and every mirror runs
  `real_leaves_unchanged()` and confirmed cleanup/copy-back before drop.
- `AGENTS.md`, "Tests and validation", `.agents/test-authoring.md` (apply during authoring and supply verbatim to the
  reviewer), `.agents/validation.md`, `.agents/test-evidence.md`, and `docs/test-sleep-check.md`
  (`python -B scripts/check-test-sleeps.py`).
- `AGENTS.md`, "Conventional Commits" and "Changelog fragments".
- `docs/harness-drift-tests.md` and `.agents/real-agent-tui.md`: live drift checks use real vendor logins on Linux,
  report missing auth as `COULD NOT RUN`, and stay `#[ignore]` in ordinary CI/nextest.
- `plans/AGENTS.md` and `plans/.agents/executing.md`: no absolute paths in repo files, never edit `plans/`, keep PRs as
  drafts, and never ask the maintainer questions in conversation.

### Planner-proposed implementation choices

- **Skip an unconsented live launch phase:** Whether Farhelm skips installing hook files when
  `setup_persistent_hooks = false` is a local Farhelm guard with no vendor CLI involvement and is already unit-tested in
  `crates/farhelm-supervisor/src/service/core/tests/persistent_hooks.rs`
  (`persistent_hook_install_respects_consent_and_host_opt_out`). Do not add an extra live vendor startup for it.
- **Narrow `DriftEnv::with_plain_session` in `drift/mod.rs`:** `DriftEnv::with_session` rejects
  `SessionLaunch::plain_command` (because `launch.agent_type()` is `None`) and always toggles `setup_persistent_hooks`.
  Extract the create + `SessionCleanup::check_and_delete` tail of `with_session` into a private helper and add a small
  `DriftEnv::with_plain_session(command, check)` method that skips the harness check and hook-setup flag while keeping
  `SessionCleanup` accounting intact so `PrivateConfig::finish` copy-back remains gated on confirmed Delete.
- **Promote `read_child_proc` and `hook_invisible` from `capture_timing_hooks.rs` into `drift/mod.rs`:** Move
  `read_child_proc` (with its `child_root()` revalidation) and `hook_invisible` plus an exhaustive
  `hook_protocol_reply(LaunchHarness) -> Option<&'static str>` into `drift/mod.rs` (removing `hook_reply` from
  `capture_timing_hooks::HarnessChecks`), so `capture_timing_hooks.rs` and `persistent_hooks.rs` share one
  implementation. Keep the `FARHELM_*`-absence + mirror-env predicate local to `persistent_hooks.rs`.
- **Outside-Farhelm command shape and safety:**
  - Build the vendor invocation the way a user runs it outside Farhelm (the profile's binary plus model/effort flags
    from a small exhaustive helper; do not copy Farhelm's compiled launch verbatim unless needed — note for Grok, verify
    whether plain `grok` with `GROK_HOME=<mirror>` keeps its leader isolated inside `<mirror>` rather than attaching to
    a host-wide leader; if plain `grok` shares a host leader outside `GROK_HOME`, pass `--no-leader` and document why as
    a `DECISION`).
  - Wrap it in a `bash -c` command that generically unsets every variable starting with `FARHELM_` (e.g., iterating
    `compgen -e FARHELM_`), explicitly exports the mirror's `GROK_HOME` or `HOME` after login-shell startup (so rc files
    cannot restore the maintainer's real path), and `exec`s the vendor CLI so `child_root()` is the vendor CLI process.
  - Check `/proc/<child_root>/cmdline` and `/proc/<child_root>/environ` via `read_child_proc` as a hard precondition
    before typing anything (proving zero `FARHELM_*` variables and the mirror root are active).
  - Note in code comments that stripping `FARHELM_SESSION_ID` disables the env-marker fallback sweep on Delete, relying
    on the per-launch systemd/cgroup scope and `child_root` tree walk for cleanup on Linux.
  - Do not assert vacuous checks on the outside run (with all `FARHELM_*` stripped, the hook has no destination and
    writes no log anywhere; Phase 1 against the same mirror is the discovery witness, and Phase 2 proves composer,
    rendered reply, silence/no leaked output, live process, and untouched real hook files).

## Implementation outline

1. **PR 1 — Shared helpers (`with_plain_session`, `read_child_proc`, `hook_invisible`) and `persistent_hooks` drift
   category:**
   - Lift `read_child_proc` and `hook_invisible` (with exhaustive `hook_protocol_reply`) from
     `drift/capture_timing_hooks.rs` into `drift/mod.rs`, and add `DriftEnv::with_plain_session` sharing
     `SessionCleanup` accounting with `with_session`.
   - Create `crates/farhelm/tests/e2e/real_agent_capture/drift/persistent_hooks.rs` and wire `mod persistent_hooks;` in
     `drift/mod.rs`.
   - Implement one shared `execute(harness)` function driving Phase 1 (consented install + `check_hook_setup(true)` +
     short no-tools turn capture + `hook_invisible`) and Phase 2 (plain outside-Farhelm run on the same mirror with
     pre-keystroke `environ`/`cmdline` precondition + composer + split-nonce no-tools turn via shared
     `send_prompt`/`wait_for_reply` + `hook_invisible` + untouched real hook files).
   - Add non-ignored unit tests: exhaustive guard verifying `HARNESSES` matches every built-in harness with
     `AgentKind::offers_persistent_hooks()`, plus unit tests for the `FARHELM_*`-absence + mirror-env `environ` check
     and outside-Farhelm command builder.
   - Update `docs/harness-drift-tests.md` and remove this entry from `TODO.md`.

(Split into two PRs — `drift/mod.rs` helper lifts first, `persistent_hooks.rs` second — if that makes review cleaner.)

## Scope reassessment

Before implementing a substantial departure from the outline above, run a fresh-context scope reassessment review at
your own model and effort with this exact charter (supplying the original request, settled decisions, outline, current
diff, and proposed departure — what changed, why it is necessary, and which simpler alternative was ruled out):

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

Repeated corrective reviews of the same component also require reassessing its necessity before another repair round. A
close reuse-versus-duplication call that planning did not foresee is a routine fork when every option stays within the
agreed scope: decide it after reassessment and log it as `DECISION`. Give any same-model sub-agent bounded units within
the outline; unapproved new mechanisms are outside its task and, if discovered mid-unit, are left unimplemented and
reported back honestly rather than built merely to finish the unit. If a sub-part turns disproportionately complicated,
apply the maintainer's complexity fallback above (defer that sub-part to `TODO.md` under `## Plan follow-ups` with an
explanation of what got complicated and why).

## Checkout, resume protocol, and plan record

- **Checkout check:** Run inside a checkout of `scode/farhelm` (the nearest Git worktree or jj workspace containing your
  working directory; a `.git` file counts as well as a `.git` directory). Never fall back to an enclosing checkout.
  Confirm a remote points at `scode/farhelm` before doing anything else; if not, stop immediately without writing any
  log or file and report the mismatch. All project paths in this plan are relative to the checkout the executing session
  runs in.
- **Resume protocol:** Right after confirming the checkout and claiming the plan per `plans/.agents/executing.md`,
  resolve the plan's record directory with `cargo xtask records path plan/drift-persistent-hooks`
  (`plans/.agents/outside-files.md`, "Files outside the repository"). Invoke the `agent-resumeable` skill anchored at
  `<record-dir>/log.md`. If that log already exists, read it in full and cross-check it against GitHub open PRs and VCS
  state before continuing rather than starting over. If it does not exist, this is a fresh start.

## Execution mode

- **No-workhorse mode (explicit):** Do not send any unit of your own decomposition to a weaker model or lower effort
  than your own. Do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point. If
  `scode-galaxy-brain` is already active when this run starts, this goal file is the express instruction to stop it.
- Same-model, same-effort sub-agents are allowed when they help finish sooner without costing quality (launched natively
  or via `$scode-harness-shellout`; concurrent writers use separate worktrees or jj workspaces while you own the
  `jjstack` stack and every PR).
- **Exemptions:** The three fresh-context read-only sub-agents required by `plans/AGENTS.md`, "Sub-agents this file
  requires" (resume check, cold read of a Blocked question, cold read of a report), the per-PR review gate below, and
  any scope reassessment review are explicitly exempt from the no-delegation rules.

## Resource watchdog

Right after the resume protocol and before any delegation or heavy build, start the `scode-build-goal` resource watchdog
(`scripts/watchdog` next to the `scode-build-goal` skill's `SKILL.md`, resolved with `realpath`).

- **Status file:** `farhelm-plan-drift-persistent-hooks-watchdog.json` in the parent directory of the executing checkout
  (derived per `scode-build-goal`'s "Placing the files"), never inside the `plan/<slug>` record directory.
- **Watched paths (`--path`):** the root of the executing checkout, `$HOME`, `/tmp`, the parent directory holding the
  status file, your session's scratch directory (`AGENTS.md`, "Agent scratch space"), and any worktree/build directory
  later created on another filesystem. Interval: 60 seconds; default alert thresholds.
- **Verification and lifecycle:** Follow `scode-build-goal`'s Resource watchdog rules for your harness (Claude Code
  Monitors, Codex `wake-on-exit`, or plain background sampler fallback), run the throwaway `--interval 5` verification
  first, run `watchdog check --status <file>` before workload launches and at natural boundaries, act immediately on
  `ALERT`/`WORSE`/`STALE` by pausing launches and reclaiming owned build/worktree space, record handles in the working
  log, and stop owned monitors at close.

## Stack, PR discipline, and validation

- Base the stack on `main@origin` (or the plan's own open `plan/drift-persistent-hooks/*` PRs when resuming), never
  another plan's PRs, per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmarks `plan/drift-persistent-hooks/<nn>-<short-name>`. Keep every PR a draft; never mark a PR
  ready and never merge. Restructure PRs in place rather than stacking fixups on top (including open PRs from an earlier
  run). Never edit `plans/`.
- Follow `AGENTS.md`, "Conventional Commits" and "Changelog fragments".
- Apply `.agents/test-authoring.md` during authoring, run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and run targeted validation per `.agents/validation.md` and `.agents/test-evidence.md`
  through `python3 scripts/record-test-run.py` (unit/guard tests in `e2e`, `scripts/test-run-harness-drift.py`,
  `cargo clippy --tests`, `dprint check`).
- **Authorized live drift runs:** This plan explicitly authorizes running the opt-in live drift suite with
  `--no-upgrade` (never drop `--no-upgrade`: upgrading mutates the installed agent CLIs shared by concurrent agents on
  this machine). Run `python3 -B scripts/run-harness-drift.py --no-upgrade --category persistent_hooks` where
  installed/authenticated and record actual `PASSED` or `COULD NOT RUN` verdicts honestly in the report.

## Review gate

Before finishing each PR, delegate an independent review to a **single fresh-context Opus 5.5 agent at high effort**
(launched natively or via `$scode-harness-shellout`; do not hardcode a CLI command here). Give it:

- The repo root (filled in at run time), the commit range or bookmark to review, and a private findings file path in
  your scratch directory.
- The full text of `.agents/test-authoring.md` verbatim.
- This general review charter: review the PR's changes for general correctness (bugs, unhandled cases, broken
  invariants, wrong behavior against this plan's acceptance criteria), design (whether the shape of the change fits the
  codebase and the goal, and whether a simpler shape would), and idiomatic code for the languages involved; report
  findings to the named file, most severe first, with the file, the quoted code, what is wrong, and the smallest fix;
  edit nothing and touch no VCS state.

Address all findings before moving on.

## Decision logging and unattended fallback

- Log every consequential design or implementation choice in the `plan/drift-persistent-hooks` working log with a
  `DECISION` label and the alternatives considered.
- **Unattended means no questions:** Never ask the maintainer anything in conversation — no direct questions, no harness
  question prompts, no approval prompts, and no "want me to ...?" offers. Resolve routine forks within scope and log
  them as `DECISION`. If a sub-part becomes disproportionately complicated, apply the agreed complexity fallback (record
  a `## Plan follow-ups` entry in `TODO.md` explaining what got complicated and why, and note it in the report). For any
  material scope expansion, weakened guarantee, or design conflict not covered by an agreed fallback, block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Things the maintainer should know and optional
  follow-ups belong in the delivery report.

## Done criterion

Every PR in the stack exists as a draft, has passed its Opus 5.5 high review gate, and passes its targeted validation
checks; the `TODO.md` entry is removed in the last code PR (with any complexity-deferred piece recorded under
`## Plan follow-ups`); and the latest `## Decisions` entry, if any, is satisfied. Delivering the report is the
executor's closing step, not part of this goal.

## Release note

No release note needed.

# Plan: Base launch, Codex process isolation, and Cursor/OpenCode interactive turns

## Goal

Extend the opt-in real-agent drift test suite (`crates/farhelm/tests/e2e/real_agent_capture/drift/`) to verify Codex's
`--no-daemon` process-tree isolation inside the `base` category and to send a real interactive prompt turn to Cursor and
OpenCode in `base` instead of stopping at their initial TUI or trust screen.

Original `TODO.md` entry (under `## Near term` → `### Drift test feature coverage`):

> **Drift tests: base launch, Codex process isolation, and Cursor/OpenCode interactive turns.** Verify that Codex's
> `--no-daemon` keeps hooks and commands inside the session's process tree and still launches cleanly when hooks are
> disabled, verify Cursor's default launch without a permission flag ("Agent's own setting"), and send a real prompt to
> Cursor and OpenCode to confirm interactive turns work under Farhelm's terminal host (today both stop at the initial
> TUI or trust screen).

### Acceptance criteria

- `base/codex` proves that Codex's `--no-daemon` keeps a real shell command run by Codex inside the session's pinned
  process tree (not detached into an external daemon), verified through a dedicated YOLO probe session after the
  existing Guarded recall turns, and validated once during development with a throwaway negative control without
  `--no-daemon`. Hook-in-tree isolation is already proven by `base/codex`'s existing `HookWitness::CommittedIdentity`
  turns (since the supervisor refuses Codex reports whose `/proc` ancestry does not reach the session's pane), and
  hooks-off Codex launching cleanly with `--no-daemon` remains verified by `capture_timing_hooks/codex`; document both
  in `base.rs` and `docs/harness-drift-tests.md` rather than duplicating them.
- `base/cursor` and `base/opencode` auto-answer workspace trust prompts (if shown), submit one short no-tools prompt
  with a split nonce, and confirm the rendered reply on screen under Farhelm's terminal host (giving default-run
  coverage in `base`, separate from `named_conversation`), while still reporting `COULD NOT RUN` when vendor logins or
  credentials are missing.
- Shared untracked prompt/reply helpers (`send_prompt`, `wait_for_reply`) live in
  `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs` and are reused by `named_conversation.rs`, `base.rs`'s
  Cursor/OpenCode turns, and `base.rs`'s two inline tracked-turn submits. Harness differences stay in small exhaustive
  `#[warn(clippy::wildcard_enum_match_arm)]` functions.
- Non-ignored unit tests pin the new `/proc/<pid>/stat` ancestry parser and `ProcessRoot` matcher (private in
  `drift/base.rs`) without starting real vendor CLIs.
- `docs/harness-drift-tests.md` reflects the updated `base` behavior for Codex, Cursor, and OpenCode.
- The last code PR removes this entry from `TODO.md`.

## Requirement sources

### Maintainer request and settled decisions

- **Drop Cursor's removed "Agent's own setting" sub-item:** Cursor's "Agent's own setting" permission mode no longer
  exists (`omitted_permission(LaunchHarness::Cursor)` is `Some(LaunchPermission::Yolo)` and `offers_permission` accepts
  only `Yolo` and `Custom`). `base/cursor` already exercises Cursor's default YOLO launch and `permission_modes/cursor`
  covers YOLO and Custom; do not invent a replacement non-force check.
- **Verify Codex hook and command process-tree isolation inside `base/codex`:** Keep the check in `base/codex` rather
  than adding a separate category.
- **Run a real interactive turn for Cursor and OpenCode in `base`:** Switch `base/cursor` and `base/opencode` from
  stopping at the initial TUI/trust prompt to auto-answering trust and sending one short no-tools prompt so the runner's
  default category exercises live interactive turns for both.
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
- `AGENTS.md`, "Tests and validation", `.agents/test-authoring.md` (apply during authoring and supply verbatim to the
  reviewer), `.agents/validation.md`, `.agents/test-evidence.md`, and `docs/test-sleep-check.md`
  (`python -B scripts/check-test-sleeps.py`).
- `AGENTS.md`, "Conventional Commits" and "Changelog fragments".
- `docs/harness-drift-tests.md` and `.agents/real-agent-tui.md`: live drift checks use real vendor logins on Linux,
  report missing auth as `COULD NOT RUN`, and stay `#[ignore]` in ordinary CI/nextest.
- `plans/AGENTS.md` and `plans/.agents/executing.md`: no absolute paths in repo files, never edit `plans/`, keep PRs as
  drafts, and never ask the maintainer questions in conversation.

### Planner-proposed implementation choices

- **Shared `send_prompt` and `wait_for_reply` in `drift/mod.rs`:** Move `send_prompt` and `wait_for_reply` from
  `drift/named_conversation.rs` into `drift/mod.rs` (generalizing `wait_for_reply`'s `"named-start"` error message to a
  phase label or generic message). Reuse them in `named_conversation.rs`, in `base.rs`'s Cursor/OpenCode turns, and in
  `base.rs`'s two inline tracked-turn submits (Turn 1 and post-Restart Turn 2). Leave `permission_modes.rs`'s
  `check_custom_restart` alone (it checks for prompt echo after typing, which is a different contract). For
  `base/cursor` and `base/opencode`, use a split-nonce prompt (like `capture_timing_hooks.rs`:
  `"Join FH and {nonce} without spaces. Reply with only that word. No tools."`) so OpenCode's narrow composer wrapping
  can never echo the answer token.
- **Codex `--no-daemon` process-tree isolation probe (private in `drift/base.rs`):**
  - _Hook in tree:_ Add no new code. `base/codex` already runs with `HookWitness::CommittedIdentity` and the supervisor
    (`service/report_files.rs` and `service/core/vendor/codex.rs`) already refuses any Codex hook report whose `/proc`
    ancestry does not reach the session's terminal anchor. Document that in `base.rs` and `docs/harness-drift-tests.md`.
  - _Command in tree — second `with_drift_supervisor` call with `LaunchPermission::Yolo`:_ Keep `base/codex`'s existing
    Guarded recall turns unchanged. Run a second `with_drift_supervisor` call with
    `permissions: Some(LaunchPermission::Yolo)` (interactive in the terminal host) enabled via an exhaustive per-harness
    check in `base.rs` (`#[warn(clippy::wildcard_enum_match_arm)]`, true only for `Codex`). YOLO is required for two
    reasons: avoiding shell-command approval dialogs AND avoiding Codex's `-s workspace-write` sandbox, which can hide
    or unshare `/proc` PIDs.
  - _Probe mechanics:_ Verify `--no-daemon` is present in `child_root()`'s `/proc/<pid>/cmdline` (revalidating
    `child_root()`). Write a tiny `record-tree.sh` script in the scratch workspace that walks `p=$$` up to PID 1 via
    `ps -o ppid= -p "$p" | tr -d ' '`, appends each raw `/proc/$p/stat` line to `tree.txt.tmp`, and atomically renames
    `tree.txt.tmp` to `tree.txt`. Ask Codex to run `sh ./record-tree.sh` and poll for `tree.txt` (calling
    `terminal.check(...)`, never waiting on fragile model prose). Parse each `/proc/<pid>/stat` line in Rust after
    `rsplit_once(')')` (handling `comm` names containing spaces or `)`) and match `ProcessRoot { pid, start }` against
    the recorded chain. Keep the parser private in `drift/base.rs` (single caller).
  - _Negative-control verification during development:_ Because a freshly auto-started Codex daemon via `setsid()` could
    still have `child_root` as an ancestor unless Codex double-forks, prove once during development (throwaway manual
    check, not committed as a test) with an undeclared `SessionLaunch::plain_command` running `codex` without
    `--no-daemon` whether PID+start-in-chain discriminates. Record the observation in the plan's working log. If bare
    PID-in-chain passes without `--no-daemon`, add the cheapest discriminator already present in the dumped `stat` lines
    (such as Unix session ID, field 6, verified against the installed Codex's command-child session layout); if
    discriminating gets disproportionately fiddly, apply the complexity fallback and document the exact limitation in
    `base.rs`.
- **Hooks-off Codex `--no-daemon` check:** Already covered by `drift/capture_timing_hooks.rs`
  (`hooks_off_child_words: &["--no-daemon"]` plus a live hooks-off turn). Do not duplicate it in `base.rs`.

## Implementation outline

1. **PR 1 — Shared prompt/reply helpers in `drift/mod.rs` and Cursor/OpenCode interactive turns in `base`:**
   - Move `send_prompt` and `wait_for_reply` from `drift/named_conversation.rs` into `drift/mod.rs` with generic error
     wording and reuse them across `named_conversation.rs` and `base.rs` (both tracked Turn 1 / Turn 2 and untracked
     Cursor / OpenCode).
   - Switch launch-only harnesses (`hook_witness.is_none()`, i.e., `Cursor` and `OpenCode`) in `drift/base.rs` to
     `TrustPolicy::AutoAnswer` and one short split-nonce no-tools turn.
2. **PR 2 — Codex `--no-daemon` process-tree isolation probe in `base/codex`, docs, and TODO removal:**
   - Add the private `/proc/<pid>/stat` ancestry parser and `ProcessRoot { pid, start }` matcher in `drift/base.rs`,
     backed by non-ignored unit tests (synthetic chains, spaces/`)` in `comm`, start-time mismatch, detached PID-1
     chain).
   - Add the exhaustive per-harness isolation check hook in `drift/base.rs` running the second YOLO
     `with_drift_supervisor` probe session for `Codex` (after proving discrimination against the throwaway negative
     control during development and recording the result in the working log).
   - Update `docs/harness-drift-tests.md` and remove this entry from `TODO.md`.

(If the executor judges one cohesive PR is cleaner than two without over-sizing review, a single PR is acceptable.)

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
  resolve the plan's record directory with `cargo xtask records path plan/drift-base-isolation-and-turns`
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

- **Status file:** `farhelm-plan-drift-base-isolation-and-turns-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`'s "Placing the files"), never inside the `plan/<slug>` record directory.
- **Watched paths (`--path`):** the root of the executing checkout, `$HOME`, `/tmp`, the parent directory holding the
  status file, your session's scratch directory (`AGENTS.md`, "Agent scratch space"), and any worktree/build directory
  later created on another filesystem. Interval: 60 seconds; default alert thresholds.
- **Verification and lifecycle:** Follow `scode-build-goal`'s Resource watchdog rules for your harness (Claude Code
  Monitors, Codex `wake-on-exit`, or plain background sampler fallback), run the throwaway `--interval 5` verification
  first, run `watchdog check --status <file>` before workload launches and at natural boundaries, act immediately on
  `ALERT`/`WORSE`/`STALE` by pausing launches and reclaiming owned build/worktree space, record handles in the working
  log, and stop owned monitors at close.

## Stack, PR discipline, and validation

- Base the stack on `main@origin` (or the plan's own open `plan/drift-base-isolation-and-turns/*` PRs when resuming),
  never another plan's PRs, per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmarks `plan/drift-base-isolation-and-turns/<nn>-<short-name>`. Keep every PR a draft; never
  mark a PR ready and never merge. Restructure PRs in place rather than stacking fixups on top (including open PRs from
  an earlier run). Never edit `plans/`.
- Follow `AGENTS.md`, "Conventional Commits" and "Changelog fragments".
- Apply `.agents/test-authoring.md` during authoring, run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and run targeted validation per `.agents/validation.md` and `.agents/test-evidence.md`
  through `python3 scripts/record-test-run.py` (unit/guard tests in `e2e`, `scripts/test-run-harness-drift.py`,
  `cargo clippy --tests`, `dprint check`).
- **Authorized live drift runs:** This plan explicitly authorizes running the opt-in live drift suite with
  `--no-upgrade` (never drop `--no-upgrade`: upgrading mutates the installed agent CLIs shared by concurrent agents on
  this machine). Run
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category base --harness codex --harness cursor --harness opencode`
  (and `--category capture_timing_hooks --harness codex`) where installed/authenticated; record actual `PASSED` or
  `COULD NOT RUN` verdicts honestly in the report without claiming live verification that did not run.

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

- Log every consequential design or implementation choice in the `plan/drift-base-isolation-and-turns` working log with
  a `DECISION` label and the alternatives considered.
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

# Plan: Permission-mode status-line footer detection drift tests

## Goal

Extend the `permission_modes` drift category (`crates/farhelm/tests/e2e/real_agent_capture/drift/permission_modes.rs`)
so it verifies Farhelm's live screen-footer permission classifier on both YOLO and Guarded launches for Claude, Grok,
and Muse, and on YOLO (`--force`) launches for Cursor — explicitly distinguishing a recognized non-YOLO footer
(`Some(false)`) from an unreadable/drifted footer (`None`).

Original `TODO.md` entry (under `## Near term` → `### Drift test feature coverage`):

> **Drift tests: permission-mode status-line footer detection.** Verify Farhelm's live screen-footer classifier reports
> YOLO vs non-YOLO accurately on both YOLO and Guarded launches for Claude, Grok, and Muse, and YOLO on `--force`
> launches for Cursor, catching vendor footer wording or layout drift.

### Acceptance criteria

- `farhelm-supervisor` exposes a minimal `#[cfg(feature = "test-seams")]` API for the permission-footer classifier:
  - `status_line_yolo_for_test(harness: LaunchHarness, raw_screen: &str) -> Option<bool>` trimming `raw_screen` with the
    sampler's `SAMPLE_TAIL_BYTES` before calling `status_line_yolo`.
  - A concrete exhaustive `StatusLineYoloSupport { None, YoloOnly, YoloAndNonYolo }` enum and
    `status_line_yolo_support_for_test(LaunchHarness) -> StatusLineYoloSupport` defined right beside `status_line_yolo`
    in `screen_reader.rs`, derived from by **both** `status_line_fixtures.rs` (replacing its hard-coded four-harness
    list) and `permission_modes.rs` (replacing its hand-mirrored `reads_yolo` list).
- `drift/permission_modes.rs` runs both `LaunchPermission::Yolo` and `LaunchPermission::Guarded` for `Claude`, `Grok`,
  and `Muse`, and `LaunchPermission::Yolo` (`--force`) for `Cursor`, via one shared footer-verification helper:
  - On YOLO launches for all four harnesses: polls until `status_line_yolo_for_test(harness, &screen) == Some(true)` AND
    `session.screen_yolo == true`.
  - On Guarded launches for Claude, Grok, and Muse: polls until
    `status_line_yolo_for_test(harness, &screen) == Some(false)` (proving the non-YOLO footer was recognized, not
    unreadable `None`), followed by one `list_sessions()` sanity check confirming `session.screen_yolo == false` (no
    artificial hold/sleep window; production latching on `Some(true)` only is already unit-tested in `ticker.rs`).
- Non-ignored unit tests pin both `status_line_fixtures.rs` (iterating `LaunchHarness::ALL` against
  `StatusLineYoloSupport`, requiring fixture directories and `not-yolo-*` grids accordingly) and `permission_modes.rs`
  (verifying every `YoloOnly`/`YoloAndNonYolo` harness includes the matching `Yolo` and, where offered, `Guarded` mode
  entries).
- Shared code carries the mode dispatch and footer polling loop; per-harness details stay in small exhaustive
  `#[warn(clippy::wildcard_enum_match_arm)]` functions.
- `docs/harness-drift-tests.md` reflects the added Guarded and tri-state footer verification in `permission_modes`.
- The last code PR removes this entry from `TODO.md`.

## Requirement sources

### Maintainer request and settled decisions

- **Extend `permission_modes` (no new category):** Reuse `permission_modes`'s existing YOLO launches for Claude, Grok,
  Muse, and Cursor and add Guarded launches alongside them.
- **Scoped to footer-classified harnesses only:** Add Guarded launches only for Claude, Grok, and Muse (the three
  harnesses with a non-YOLO footer classifier), keeping Cursor on YOLO (`--force`) and leaving Codex, Antigravity, and
  OMP unchanged in `permission_modes`.
- **Maximize shared code:** Use shared code as much as possible and plug in harness-specific code only where required.
- **No line-count gates; complexity fallback:** Do not enforce artificial line-count gates. If any part of this plan
  turns disproportionately complicated during execution, stop sinking complexity into it, leave that piece out, and add
  a `## Plan follow-ups` entry in `TODO.md` (`plans/AGENTS.md`, "Follow-up TODO entries") explaining what got
  complicated and why.
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter.

### Binding repository constraints

- `AGENTS.md`, "Code rules" and `crates/farhelm-supervisor/src/agent_kind/mod.rs`: exhaustive per-harness dispatch with
  `#[warn(clippy::wildcard_enum_match_arm)]` (no `kind == X`, `matches!`, or `_` arms in shared code).
- `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs` (`status_line_yolo`, `SAMPLE_TAIL_BYTES`) and
  `crates/farhelm-supervisor/src/service/ticker.rs` (`observe_permission`): the sampler feeds the 4096-byte trimmed
  screen tail (`farhelm_terminal_emulator::trim_screen_tail(..., SAMPLE_TAIL_BYTES)`) into `status_line_yolo` and
  latches `screen_yolo = true` only on `Some(true)`. `client.list_sessions()` reads the cached cell without sampling
  screens, and `reconcile_for_test()` never samples screens. Do not change production latching or wire protocol for a
  test-only distinction, and do not add fixed sleeps to wait for the 2-second ticker on Guarded launches.
- `AGENTS.md`, "Tests and validation", `.agents/test-authoring.md` (apply during authoring and supply verbatim to the
  reviewer), `.agents/validation.md`, `.agents/test-evidence.md`, and `docs/test-sleep-check.md`
  (`python -B scripts/check-test-sleeps.py`).
- `AGENTS.md`, "Conventional Commits" and "Changelog fragments".
- `docs/harness-drift-tests.md` and `.agents/real-agent-tui.md`: live drift checks use real vendor logins on Linux,
  report missing auth as `COULD NOT RUN`, and stay `#[ignore]` in ordinary CI/nextest.
- `plans/AGENTS.md` and `plans/.agents/executing.md`: no absolute paths in repo files, never edit `plans/`, keep PRs as
  drafts, and never ask the maintainer questions in conversation.

### Planner-proposed implementation choices

- **Single source of truth for footer-classifier support (`StatusLineYoloSupport`):** In
  `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs` (re-exported from `agent_kind/mod.rs` under
  `#[cfg(feature = "test-seams")]`, and available to internal unit tests via
  `#[cfg(any(test, feature = "test-seams"))]`), define:
  - `pub enum StatusLineYoloSupport { None, YoloOnly, YoloAndNonYolo }`
  - `pub fn status_line_yolo_support_for_test(harness: LaunchHarness) -> StatusLineYoloSupport`
  - `pub fn status_line_yolo_for_test(harness: LaunchHarness, raw_screen: &str) -> Option<bool>` (trimming via
    `farhelm_terminal_emulator::trim_screen_tail(raw_screen, SAMPLE_TAIL_BYTES)` before calling `status_line_yolo`).
    Update `crates/farhelm-supervisor/src/agent_kind/status_line_fixtures.rs` so
    `captured_permission_footers_match_and_ignore_transcript_mode_words` iterates `LaunchHarness::ALL` using
    `status_line_yolo_support_for_test`, requiring a fixture directory whenever support is not `None` and requiring
    `not-yolo-*` grids whenever support is `YoloAndNonYolo`. That makes the capability enum genuinely authoritative
    instead of a third hand-maintained list.
- **Single shared footer waiter in `drift/permission_modes.rs`:** Replace `wait_for_yolo` with
  `wait_for_permission_footer(terminal, id, harness, expected: bool)` polling up to the existing 15-second deadline:
  - reads `terminal.host.screen_tail(SCREEN_BYTES)` and checks
    `status_line_yolo_for_test(harness, &screen) == Some(expected)`;
  - for `expected == true`: also requires `session.screen_yolo == true` from `terminal.client.list_sessions()` (waiting
    for the sampler to latch);
  - for `expected == false`: once `status_line_yolo_for_test == Some(false)` holds, performs one `list_sessions()` read
    confirming `session.screen_yolo == false` as a sanity check (no sleep/hold window);
  - on timeout, reports the last observed `Option<bool>` reading (`None` vs `Some(!expected)`), `session.screen_yolo`,
    and `terminal.diagnostics()` so vendor wording drift is immediately legible.
- **Adding Guarded entries in `modes(harness)`:** Gate `choice(Guarded, harness.guarded_flags())` on
  `status_line_yolo_support_for_test(harness) == StatusLineYoloSupport::YoloAndNonYolo && harness.offers_permission(LaunchPermission::Guarded)`
  (reusing `LaunchHarness::guarded_flags()` directly rather than re-typing flags, and keeping future non-Guarded
  harnesses safe).

## Implementation outline

1. **PR 1 — Supervisor `test-seams` footer helper, `permission_modes` Guarded/tri-state checks, docs, and TODO
   removal:**
   - Add `StatusLineYoloSupport`, `status_line_yolo_support_for_test`, and `status_line_yolo_for_test` in
     `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs` and re-export from `agent_kind/mod.rs`.
   - Update `crates/farhelm-supervisor/src/agent_kind/status_line_fixtures.rs` to drive its fixture-directory and
     `not-yolo-*` grid assertions from `status_line_yolo_support_for_test` over `LaunchHarness::ALL`.
   - Update `crates/farhelm/tests/e2e/real_agent_capture/drift/permission_modes.rs`:
     - Delete the hand-mirrored `reads_yolo` table in favor of `status_line_yolo_support_for_test`.
     - Add `LaunchPermission::Guarded` (using `harness.guarded_flags()`) to `modes(harness)` when
       `StatusLineYoloSupport::YoloAndNonYolo` and `harness.offers_permission(LaunchPermission::Guarded)` both hold.
     - Replace `wait_for_yolo` with `wait_for_permission_footer`.
     - Extend unit tests in `permission_modes.rs` to pin that every `YoloOnly`/`YoloAndNonYolo` harness includes the
       matching `Yolo` and (where offered) `Guarded` mode entries.
   - Update `docs/harness-drift-tests.md` and remove this entry from `TODO.md`.

(This is small and cohesive enough for one PR; split into two PRs only if the executor finds the supervisor seam
deserves its own review unit.)

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
  resolve the plan's record directory with `cargo xtask records path plan/drift-permission-footer`
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

- **Status file:** `farhelm-plan-drift-permission-footer-watchdog.json` in the parent directory of the executing
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

- Base the stack on `main@origin` (or the plan's own open `plan/drift-permission-footer/*` PRs when resuming), never
  another plan's PRs, per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmarks `plan/drift-permission-footer/<nn>-<short-name>`. Keep every PR a draft; never mark a PR
  ready and never merge. Restructure PRs in place rather than stacking fixups on top (including open PRs from an earlier
  run). Never edit `plans/`.
- Follow `AGENTS.md`, "Conventional Commits" and "Changelog fragments".
- Apply `.agents/test-authoring.md` during authoring, run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and run targeted validation per `.agents/validation.md` and `.agents/test-evidence.md`
  through `python3 scripts/record-test-run.py` (`farhelm-supervisor` unit tests including `status_line_fixtures`,
  `permission_modes` unit tests in `e2e`, `scripts/test-run-harness-drift.py`, `cargo clippy --tests`, `dprint check`).
- **Authorized live drift runs:** This plan explicitly authorizes running the opt-in live drift suite with
  `--no-upgrade` (never drop `--no-upgrade`: upgrading mutates the installed agent CLIs shared by concurrent agents on
  this machine). Run
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category permission_modes --harness claude --harness grok --harness muse --harness cursor`
  where installed/authenticated and record actual `PASSED` or `COULD NOT RUN` verdicts honestly in the report.

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

- Log every consequential design or implementation choice in the `plan/drift-permission-footer` working log with a
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

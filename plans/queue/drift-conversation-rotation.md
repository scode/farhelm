# Plan: In-session conversation rotation drift tests

## Goal

Add a `conversation_rotation` drift category
(`crates/farhelm/tests/e2e/real_agent_capture/drift/conversation_rotation.rs`) built on one shared rotation driver with
small exhaustive per-harness descriptors, replacing the three standalone ignored rotation tests in
`crates/farhelm/tests/e2e/real_agent_capture.rs` and covering in-session rotation across Claude, Codex, Muse, Pi, OMP,
Grok, and Antigravity so Restart resumes the rotated conversation.

Original `TODO.md` entry (under `## Near term` → `### Drift test feature coverage`):

> **Drift tests: in-session conversation rotation.** Wire the existing standalone Claude `/clear`, Codex `/new`, and
> Codex `/clear` checks into the drift runner (running Codex `/clear` end-to-end through Farhelm's own hook so pending
> withdrawal and `Stop` confirmation are tested), and add live rotation coverage for Muse (`/clear`, `/new`), Pi
> (`/new`, `/fork`, `/resume`), OMP (`/new`, `/fork`, `/resume`), Grok (`/new`), and Antigravity, proving Restart
> resumes the rotated conversation.

### Acceptance criteria

- A new `conversation_rotation` drift category is discovered by `scripts/run-harness-drift.py` and covers `Claude`,
  `Codex`, `Muse`, `Pi`, `Omp`, `Grok`, and `Antigravity` (pinned by a non-ignored exhaustive unit guard over
  `rotation_spec(LaunchHarness)` so any newly added harness must explicitly decide its rotation coverage in the
  exhaustive match).
- One shared rotation driver executes the common lifecycle across all harnesses, plugging in only harness-specific
  descriptors (commands, SPEC-grounded timing checks, and any interactive picker steps), while fitting comfortably
  within `scripts/run-harness-drift.py`'s 720-second per-harness budget:
  - **Claude:** `/clear` (preserves the replaced test's startup capture and immediate rotation before any prompt on the
    cleared chat, plus Restart resuming the rotated conversation).
  - **Codex:** `/new` and `/clear`, both running end-to-end through Farhelm's real hook (not an undeclared raw `codex`
    Python probe): preserves `/new`'s lag-until-next-prompt check (and Codex's `"Approaching rate limits"` Escape
    dismissal from `real_agent_capture.rs`), verifies `/clear` commits a pending non-resumable locator
    (`RestartOffer != Resume`, new runtime ID) before the next prompt and upgrades to `RestartOffer::Resume` at `Stop`,
    and proves Restart resumes the rotated conversation and recalls the new code word.
  - **Muse:** `/clear` and `/new`, proving a new conversation is captured and Restart recalls its code word.
  - **Pi:** `/new`, plus `/fork` and `/resume` (subject to the maintainer's complexity fallback below: if `/fork` or
    `/resume` interactive picker driving gets disproportionately complicated, leave that sub-command out and record a
    `## Plan follow-ups` TODO explaining why).
  - **OMP:** `/new`, plus `/fork` and `/resume` (same complexity fallback).
  - **Grok:** `/new` (proving Restart resumes the rotated conversation; `/fork` is explicitly unsupported per
    `website/src/content/docs/docs/agents/grok.md`).
  - **Antigravity:** `/clear` (aliased as `/new` in `agy 1.5.18`, verified live during planning:
    `> /clear (new) — Clear conversation and start a new one`; `/fork` and `/resume` are explicitly out of scope for
    Antigravity). Proves rotation after `/clear` + next prompt and Restart resuming the rotated conversation.
- The three superseded standalone rotation tests in `crates/farhelm/tests/e2e/real_agent_capture.rs`
  (`real_claude_session_reports_its_identity_across_clear`, `real_codex_session_reports_its_identity_across_new`,
  `real_codex_stop_names_the_persisted_transcript_after_clear`) and any helpers left unused once those three tests are
  removed are cleaned up, and stale module comments in `real_agent_capture.rs` are updated (keeping
  `real_claude_shelled_out_child_cannot_replace_the_session_conversation` and every helper it or `drift/` still calls
  intact).
- Non-ignored unit tests cover the exhaustive harness descriptor guard, baseline-counted slash-command matching, and
  rotation admission predicates without launching real vendor CLIs.
- `docs/harness-drift-tests.md` documents the `conversation_rotation` category.
- The last code PR removes this entry from `TODO.md` (and adds a `## Plan follow-ups` entry only if `/fork`/`/resume` or
  another piece was deferred under the complexity fallback).

## Requirement sources

### Maintainer request and settled decisions

- **Replace and remove the three standalone rotation tests in `real_agent_capture.rs`:** Once wired into
  `conversation_rotation`, remove those three standalone tests so there is one canonical shared rotation suite.
- **Pi and OMP `/fork` and `/resume`:** Include `/new`, `/fork`, and `/resume` for Pi and OMP — _"but if fork/resume get
  super complicated just leave them out and make them plan follow-up todos"_.
- **Antigravity scope (settled during planning):** Cover `/clear` (alias `/new`) only — verified live on `agy 1.5.18`
  during planning; do not probe for or add `/fork` or `/resume` for Antigravity.
- **Maximize shared code:** Use shared code as much as possible and plug in harness-specific code only where required.
- **No line-count gates; complexity fallback:** Do not enforce artificial line-count gates. If any part of this plan
  turns disproportionately complicated during execution, stop sinking complexity into it, leave that piece out, and add
  a `## Plan follow-ups` entry in `TODO.md` (`plans/AGENTS.md`, "Follow-up TODO entries") explaining what got
  complicated and why.
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter.

### Binding repository constraints

- `AGENTS.md`, "Code rules" and `crates/farhelm-supervisor/src/agent_kind/mod.rs`: exhaustive per-harness dispatch with
  `#[warn(clippy::wildcard_enum_match_arm)]` (no `kind == X`, `matches!`, or `_` arms in shared code).
- `SPEC.md` and `SPEC_impl.md`: source any intermediate pre-prompt rotation timing assertions strictly from `SPEC.md`
  and the three replaced tests' pinned vendor facts, never from guesses.
- `scripts/run-harness-drift.py`: each `category/harness` case shares a 720-second recorder timeout budget; do not raise
  the runner timeout for this plan.
- `AGENTS.md`, "Tests and validation", `.agents/test-authoring.md` (apply during authoring and supply verbatim to the
  reviewer), `.agents/validation.md`, `.agents/test-evidence.md`, and `docs/test-sleep-check.md`
  (`python -B scripts/check-test-sleeps.py`).
- `AGENTS.md`, "Conventional Commits" and "Changelog fragments".
- `docs/harness-drift-tests.md` and `.agents/real-agent-tui.md`: live drift checks use real vendor logins on Linux,
  report missing auth as `COULD NOT RUN`, and stay `#[ignore]` in ordinary CI/nextest.
- `plans/AGENTS.md` and `plans/.agents/executing.md`: no absolute paths in repo files, never edit `plans/`, keep PRs as
  drafts, and never ask the maintainer questions in conversation.

### Planner-proposed implementation choices

- **Shared rotation driver in `drift/conversation_rotation.rs`:** Define a small `RotationSpec` descriptor returned by
  an exhaustive `rotation_spec(LaunchHarness) -> Option<RotationSpec>` (`#[warn(clippy::wildcard_enum_match_arm)]`, with
  brief comments on `None` arms for `Goose`, `Cursor`, `OpenCode`, shells, and `Custom`).
- **Operation-specific recall oracles (no negative-recall turns):**
  - **Fresh rotation (`/clear`, `/new`):** Seed word A in `conv_1` -> invoke slash command -> run any pinned pre-prompt
    timing check -> seed word B in the new conversation -> verify accepted capture `conv_2 != conv_1` -> Stop + Restart
    -> confirm `RECALL-B` with `conv_2` preserved (positive recall of B from a conversation whose only history is B
    already proves the abandoned conversation was not loaded; skip negative-recall turns).
  - **Fork (`/fork`, Pi/OMP):** Seed word A in `conv_1` -> invoke `/fork` -> seed a **new** word B in the forked
    conversation -> verify accepted capture `conv_2 != conv_1` -> Stop + Restart -> confirm `RECALL-B` with `conv_2`
    preserved (a fork inherits word A from `conv_1`, so recalling B is what proves Restart resumed the fork rather than
    the parent).
  - **Resume (`/resume`, Pi/OMP):** Must switch away from a second conversation back to an earlier one — chain after
    `/new` so the session holds `conv_2` (word B) while `conv_1` (word A) exists in the workspace -> invoke `/resume`
    selecting `conv_1` -> verify accepted capture returns to `conv_1` -> Stop + Restart -> confirm `RECALL-A` with
    `conv_1` preserved.
- **Chain operations within a session to fit the 720-second budget:** Rather than launching 6 separate sessions and 9+
  turns for Pi/OMP (or 4 launches / 6 turns for Codex/Muse), chain operations inside one `with_drift_supervisor` scope
  where natural — especially `/new` -> `/resume` (since `/resume` already needs the two conversations created by `/new`)
  and `/fork` on the same workspace.
- **Keep intermediate timing assertions minimal and grounded:** Assert pre-prompt rotation timing only for the three
  replaced tests' pinned facts (Claude `/clear` immediate new capture before next prompt; Codex `/new` identity stays
  `conv_1` until next prompt; Codex `/clear` pending withdrawal `RestartOffer != Resume` before next prompt and `Stop`
  confirmation to `RestartOffer::Resume` after next prompt) plus anything `SPEC.md` explicitly guarantees. For all other
  harnesses, the shared contract is: post-rotation prompt commits the expected conversation ID and Restart recalls its
  code word.
- **Keep rotation helpers local to `conversation_rotation.rs`:** Port the baseline-counted slash-command typing/popup
  logic (from `real_claude_session_reports_its_identity_across_clear` and Codex's `"Approaching rate limits"` Escape
  dismissal from `real_codex_session_reports_its_identity_across_new`) and the Stop + drain + Restart + recall sequence
  inside `conversation_rotation.rs`, leaving `drift/base.rs` and `drift/mod.rs`'s `Terminal::type_text` untouched (only
  adding `mod conversation_rotation;` in `drift/mod.rs`). That keeps this plan completely independent of
  `drift-base-isolation-and-turns` and `drift-persistent-hooks`.
- **Reused mechanisms:** Reuse `with_drift_supervisor`, `DriftEnv::with_session`, `Terminal`, `wait_for_turn`,
  `conversation_selector`, `acknowledged_reports`, and `private_config` from `drift/mod.rs` (so Grok and Antigravity
  automatically run in isolated config mirrors).
- **Standalone cleanup in `real_agent_capture.rs`:** Remove only the three standalone rotation tests and whatever
  becomes dead according to compiler warnings (e.g., `assert_identity_stays`, `codex_probe_events`, `hook_log_lines`,
  `hook_log_has_source`, `submit_prompt`, `trusted_codex`, `CODEX_NO_UPDATE_CHECK` — verifying which are still called by
  `real_claude_shelled_out_child_cannot_replace_the_session_conversation`, which keeps `wait_for_agent_tui_ready`,
  `wait_for_reported_identity`, `assert_resume_names`, `real_agent_launch`, and `screen`), and update stale comments in
  `real_agent_capture.rs`.

## Implementation outline

1. **PR 1 — Shared `conversation_rotation` driver for slash-command rotations (Claude, Codex, Muse, Grok, Antigravity,
   plus Pi and OMP `/new`) and standalone test cleanup:**
   - Implement `crates/farhelm/tests/e2e/real_agent_capture/drift/conversation_rotation.rs` with the shared driver,
     exhaustive `rotation_spec` table, baseline-counted slash-command typing/popup helper (preserving Codex's rate-limit
     overlay dismissal), pinned intermediate checks (Claude `/clear`, Codex `/new`, Codex `/clear` pending withdrawal +
     `Stop` confirmation through Farhelm's real hook), and post-rotation Stop + Restart recall proof.
   - Wire `mod conversation_rotation;` in `drift/mod.rs`.
   - Remove the three superseded standalone rotation tests and any genuinely unused helpers from
     `real_agent_capture.rs`, and update its comments.
   - Add non-ignored unit tests for the exhaustive harness coverage guard, baseline-counted slash-command matching, and
     rotation admission predicates.
2. **PR 2 — Pi and OMP `/fork` and `/resume` rotation operations, docs, and TODO removal:**
   - Add the small Pi and OMP interactive picker callbacks for `/fork` and `/resume` inside `conversation_rotation.rs`,
     chaining `/new` -> `/resume` (and `/fork`) inside the 720-second budget with the per-operation recall oracles
     above.
   - Apply the maintainer's complexity fallback if either `/fork` or `/resume` picker driving proves disproportionately
     complicated on the installed CLIs: leave that piece out, add a `## Plan follow-ups` entry in `TODO.md` explaining
     what got complicated and why, and document the actual covered operations in `docs/harness-drift-tests.md`.
   - Update `docs/harness-drift-tests.md` and remove the original `Near term` entry from `TODO.md`.

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
  resolve the plan's record directory with `cargo xtask records path plan/drift-conversation-rotation`
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

- **Status file:** `farhelm-plan-drift-conversation-rotation-watchdog.json` in the parent directory of the executing
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

- Base the stack on `main@origin` (or the plan's own open `plan/drift-conversation-rotation/*` PRs when resuming), never
  another plan's PRs, per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmarks `plan/drift-conversation-rotation/<nn>-<short-name>`. Keep every PR a draft; never mark a
  PR ready and never merge. Restructure PRs in place rather than stacking fixups on top (including open PRs from an
  earlier run). Never edit `plans/`.
- Follow `AGENTS.md`, "Conventional Commits" and "Changelog fragments".
- Apply `.agents/test-authoring.md` during authoring, run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and run targeted validation per `.agents/validation.md` and `.agents/test-evidence.md`
  through `python3 scripts/record-test-run.py` (unit/guard tests in `e2e`, `scripts/test-run-harness-drift.py`,
  `cargo clippy --tests`, `dprint check`).
- **Authorized live drift runs:** This plan explicitly authorizes running the opt-in live drift suite with
  `--no-upgrade` (never drop `--no-upgrade`: upgrading mutates the installed agent CLIs shared by concurrent agents on
  this machine). Run `python3 -B scripts/run-harness-drift.py --no-upgrade --category conversation_rotation` where
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

- Log every consequential design or implementation choice in the `plan/drift-conversation-rotation` working log with a
  `DECISION` label and the alternatives considered.
- **Unattended means no questions:** Never ask the maintainer anything in conversation — no direct questions, no harness
  question prompts, no approval prompts, and no "want me to ...?" offers. Resolve routine forks within scope and log
  them as `DECISION`. If a sub-part becomes disproportionately complicated (including Pi/OMP `/fork` or `/resume` picker
  driving), apply the agreed complexity fallback (record a `## Plan follow-ups` entry in `TODO.md` explaining what got
  complicated and why, and note it in the report). For any material scope expansion, weakened guarantee, or design
  conflict not covered by an agreed fallback, block per `plans/.agents/executing.md`, "Executing one plan" (step 10,
  Blocking). Things the maintainer should know and optional follow-ups belong in the delivery report.

## Done criterion

Every PR in the stack exists as a draft, has passed its Opus 5.5 high review gate, and passes its targeted validation
checks; the `TODO.md` entry is removed in the last code PR (with any complexity-deferred piece recorded under
`## Plan follow-ups`); and the latest `## Decisions` entry, if any, is satisfied. Delivering the report is the
executor's closing step, not part of this goal.

## Release note

No release note needed.

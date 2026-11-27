# Plan: Make forked and app-server Codex sessions resumable

Slug: `codex-fork-and-vscode-resume`

## Goal

Close two Codex conversation-capture gaps in `farhelm-supervisor` as two separate stacked PRs (first `fork`, then
`vscode`):

1. **PR 1 (`fork`):** Accept Codex's `fork` `SessionStart` source so Codex sessions that fork a conversation (including
   pressing `f` on Codex's "This conversation is open in another app" lock screen, or in-session forks) become resumable
   with Restart.
2. **PR 2 (`vscode`):** Accept `vscode` as a root Codex transcript header `source` regardless of `originator` so
   conversations created through a Codex app-server (such as the shared daemon, desktop app, or VS Code extension)
   become resumable in Farhelm, while keeping `mcp`, `unknown`, and all object sources (`subagent`, `internal`,
   `custom`) rejected.

### Acceptance criteria

1. **PR 1 (`fork`):**
   - `is_foreground_source` in `crates/farhelm-supervisor/src/agent_kind/codex.rs` accepts `"fork"` alongside
     `"startup"`, `"resume"`, `"clear"`, and `"compact"`, while keeping `"fork"` outside the pending-clear exception in
     `crates/farhelm-supervisor/src/service/core/vendor/codex.rs`.
   - Both the hook doorway (`foreground_source_refusal` / `check_hook_report`) and supervisor admission accept
     `SessionStart` with source `"fork"` (for both initial capture and in-session rotation) and continue rejecting
     unknown sources and missing-rollout forks without clearing an existing binding.
   - `website/src/content/docs/docs/how-it-works/agent-hook-injection.md` includes `fork` in Codex's `SessionStart`
     source list.
   - A changelog fragment is added under `releasing/changelog.d/`.
2. **PR 2 (`vscode`):**
   - `parse_record` in `crates/farhelm-supervisor/src/agent_kind/codex.rs` accepts `Some("cli" | "exec" | "vscode")`
     without inspecting `originator`, while continuing to reject `"mcp"`, `"unknown"`, and all object-valued sources
     (`subagent`, `internal`, `custom`).
   - Supervisor admission (`report_codex_conversation`) and click-time Restart verification (`verify_report_only_resume`
     -> `verify_codex_resume`) accept verified `vscode` transcripts.
   - `SPEC_impl.md` is updated where it names the accepted Codex root transcript header sources (`cli`, `exec`, or
     `vscode`). `SPEC.md` and `website/src/content/docs/docs/agents/codex.md` are left unchanged.
   - A separate changelog fragment is added under `releasing/changelog.d/` and the covered entry in `TODO.md` is
     removed.
3. `crates/farhelm-fixtures/src/fake_agent/codex_conversation.rs` and `crates/farhelm/tests/e2e/codex_identity.rs` are
   left untouched (neither PR touches hook parsing, drop files, process-ancestry walking, or launch argv).

## Requirement sources

### User request and maintainer decisions

- **Original TODO entry:** "Make forked and app-server Codex conversations resumable. Two gaps keep Restart unavailable
  for Codex sessions. First, Farhelm refuses Codex's `fork` SessionStart source. Codex's 'This conversation is open in
  another app' screen offers fork as the way out, so a session that resumed a conversation held elsewhere and forked it
  never gets Restart. Second, Farhelm refuses transcripts whose header source is `vscode`, the source Codex stamps on
  conversations created through an app-server (the shared daemon, and probably the desktop app). Those conversations can
  never become resumable in Farhelm, even once nothing else holds them. Accept both. Do the `fork` fix first, and the
  `vscode` fix as its own change because it widens which conversations Farhelm captures. The upstream research and the
  incident's evidence are in `lore/2026-11-26-codex-fork-and-vscode-session-sources.md`."
- **Two-PR ordering (explicit user requirement):** PR 1 implements `fork`, PR 2 implements `vscode` separately because
  it widens which conversations Farhelm captures.
- **App-server source scope (settled):** Accept `source: "vscode"` regardless of `originator`; keep `"mcp"` and
  `"unknown"` (and all object sources) rejected.
- **Docs scope (settled):** Update `SPEC_impl.md` and
  `website/src/content/docs/docs/how-it-works/agent-hook-injection.md` only; do not edit `SPEC.md` or
  `website/src/content/docs/docs/agents/codex.md`.
- **Queue state & review gate (settled):** Planned as `[held]`; per-PR review gate is a single fresh-context Opus 5.5
  agent at high effort with the general review charter.

### Binding repository constraints

- `SPEC.md` ("Conversation capture", Codex rules) and `SPEC_impl.md` (Codex attribution and exact-record verification).
- `lore/2026-11-26-codex-fork-and-vscode-session-sources.md` is frozen historical lore (`AGENTS.md`, "Frozen material")
  — read it for context only, never edit it.
- `crates/farhelm-supervisor/src/agent_kind/mod.rs` module-doc map governs per-harness behavior (`AGENTS.md`, "Code
  rules"). Both predicates already live in `crates/farhelm-supervisor/src/agent_kind/codex.rs` as the single points of
  truth.
- `website/AGENTS.md` governs website documentation edits (unattended runs start no preview server and name changed
  pages in the report).
- `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", `.agents/validation.md`,
  `.agents/test-evidence.md`, and `.agents/test-authoring.md`.
- `plans/AGENTS.md` and `plans/.agents/executing.md` govern claim checks, record pushes, blocking, and reporting.

### Planner-proposed implementation choices (fresh-context reviewed)

- **No e2e / `fake_agent/codex_conversation.rs` changes:** Neither PR modifies `hook.rs`, drop files, process-ancestry
  walking, or launch argv, so extending the native-process e2e fixture would be overkill. Keep coverage fast and focused
  in `agent_kind/codex.rs`, doorway tests, and `service/core.rs`.
- **Avoid duplicating the PR 2 source/originator matrix across test layers:** Put the exhaustive matrix (`cli`, `exec`,
  `vscode` with `codex-tui`, non-TUI, and absent `originator`; rejections for `mcp`, `unknown`, `{"subagent": ...}`,
  `{"internal": ...}`, `{"custom": ...}`) in fast unit tests in `crates/farhelm-supervisor/src/agent_kind/codex.rs`
  (reusing/parameterizing its existing `root_record` helper), and keep `crates/farhelm-supervisor/src/service/core.rs`
  to one compact admission + `verify_report_only_resume` test reusing `seed_codex_row` + `codex_pin_report`.
- **Reuse existing `service/core.rs` helpers and pin the doorway directly in PR 1:** Reuse `seed_codex_row` and
  `codex_pin_report` (overriding `report.source = "fork".into()`) for compact admission tests (initial capture,
  in-session fork rotation, missing-rollout refusal preserving existing capture, and unknown-source rejection), and
  assert `foreground_source_refusal(ReportVendor::Codex, ...)` (or `check_hook_report`) directly alongside
  `is_foreground_report` so the doorway path from the incident is tested directly.
- **Two separate changelog fragments:** Give PR 1 and PR 2 their own fragments under `releasing/changelog.d/` so the two
  changes remain cleanly decoupled.

## Implementation outline

Two stacked PRs:

1. **PR 1 (`plan/codex-fork-and-vscode-resume/01-accept-codex-fork-source`):**
   - Add `"fork"` to `is_foreground_source` in `crates/farhelm-supervisor/src/agent_kind/codex.rs`.
   - Update `website/src/content/docs/docs/how-it-works/agent-hook-injection.md` to include `fork` in Codex's
     `SessionStart` source list.
   - Add unit tests for `is_foreground_report` and doorway `foreground_source_refusal` / `check_hook_report`, plus
     compact `service/core.rs` admission tests reusing `seed_codex_row` and `codex_pin_report`.
   - Add a changelog fragment in `releasing/changelog.d/`.
2. **PR 2 (`plan/codex-fork-and-vscode-resume/02-accept-codex-vscode-transcript-source`):**
   - Add `"vscode"` to `parse_record` in `crates/farhelm-supervisor/src/agent_kind/codex.rs`
     (`Some("cli" | "exec" | "vscode")`).
   - Update `SPEC_impl.md` where it lists the accepted root `session_meta` header sources.
   - Add the exhaustive source/originator acceptance and rejection unit-test matrix in `agent_kind/codex.rs` (reusing
     `root_record`) and a compact admission + `verify_report_only_resume` test in `service/core.rs` (reusing
     `seed_codex_row` + `codex_pin_report`).
   - Add a separate changelog fragment in `releasing/changelog.d/` and remove the covered entry from `TODO.md`.

## Scope reassessment

Before implementing a substantial departure from the outline (for example widening capture beyond `fork` and `vscode`,
adding `originator` filtering, changing pending-clear semantics for `fork`, or extending e2e fixtures), or after
repeated corrective reviews of the same component, run a fresh-context review at the executing session's own model and
effort with this charter, supplying the original request, decisions, outline, current diff, and proposed departure:

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

Apply the decision boundary in Unattended fallback below. Give any same-model sub-agents bounded units within the
outline; unapproved new mechanisms are outside their task.

## Checkout and resume protocol

1. **Checkout check:** Run inside the checkout the executing session runs in — the nearest Git worktree or jj workspace
   containing the working directory. Confirm that at least one remote targets `scode/farhelm` (comparing host, owner,
   and repo name). If not, stop before the resume protocol, write nothing, and report the mismatch. All project paths in
   this file are relative to that checkout root.
2. **Working log (`agent-resumeable`):** After claiming the plan per `plans/.agents/executing.md`, resolve the plan's
   record directory with `cargo xtask records path plan/codex-fork-and-vscode-resume` (`plans/.agents/outside-files.md`,
   "Files outside the repository"). Its `log.md` is the working log (never create a log beside the checkout). Invoke the
   `agent-resumeable` skill with that `log.md` path: if it already exists, read all of it, cross-check against GitHub
   and VCS state, and run the resume check per `plans/.agents/executing.md`, step 7; otherwise start fresh. Push records
   at the cadence required by `plans/.agents/outside-files.md`, "When records are pushed".

## Execution mode

No-workhorse mode (explicit). Do not send any unit of the executing session's own decomposition to a weaker model or a
lower effort than the executing session's own. Do not invoke, load, or follow `scode-galaxy-brain` or
`scode-model-routing` at any point; if `scode-galaxy-brain` is already active when the run starts, this goal file is the
express instruction to stop it.

Same-model sub-agents at the executing session's own model and effort are allowed at the executor's judgment (using
separate worktrees or jj workspaces if concurrent writers might conflict), while the executing session keeps ownership
of the `jjstack` stack and every PR. Launch any model not available as a native sub-agent type (including the per-PR
reviewer) through `scode-harness-shellout`.

The read-only fresh-context sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires" (the resume check,
the cold read of a Blocked question, and the cold read of the report) are always required and exempt from any delegation
restriction. The scope reassessment review runs at the executing session's own model and effort.

## Stack and PR discipline

- Base the stack on `main@origin` (or on this plan's own open `plan/codex-fork-and-vscode-resume/*` PRs when resuming,
  never on another plan's PRs), set up per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmark names `plan/codex-fork-and-vscode-resume/<nn>-<short-name>`.
- Keep every PR a draft; never mark a PR ready and never merge.
- Restructure a PR in place instead of stacking a corrective PR on top (including open PRs built by an earlier run).
- Never edit anything under `plans/` in code PRs.
- Before every push of a plan bookmark, follow `plans/.agents/executing.md`, step 9 (`queue status` claim check, push
  records, push bookmark, log pushed commits).
- Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments", and "Tests and validation" (plus
  `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, `website/AGENTS.md`, and
  `plans/.agents/executing.md`, step 8 for noting any departure from `## Release note` in the changelog fragment).
- Remove the covered `TODO.md` entry in the last code PR (PR 2).

## Review gate

Before finishing any PR, delegate an independent review of its changes to a **single fresh-context Opus 5.5 agent at
high effort** (explicit demand for this model and effort, launched natively or through `scode-harness-shellout`; do not
hardcode a launch command). Supply the checkout root, commit range or bookmark, a private findings file in the
executor's scratch directory, and this complete charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

Address every valid finding and re-verify before finishing the PR.

## Resource watchdog

Right after the resume protocol and before any delegation or heavy build, start the `scode-build-goal` resource watchdog
(resolved via `realpath` from `scripts/watchdog` next to the installed `scode-build-goal` skill's `SKILL.md`; requires
`deno`).

- **Status file:** `farhelm-plan-codex-fork-and-vscode-resume-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files"), never inside the `plan/<slug>` record directory.
- **Watched paths (`--path`):** the executing checkout root, the home directory, `/tmp`, the parent directory holding
  the status file, the executing session's scratch directory under `/tmp/farhelm-tmp/<id>/`, and any worktree or build
  directory on another filesystem. Interval: 60 seconds, default alert levels (filesystem under 5 GiB free or under 25%
  for filesystems < 20 GiB, or under 10% free while also < 20 GiB; memory/cgroup under 10% available while also < 4
  GiB).
- **Harness mechanics, verification, acting on alerts, and lifecycle:** Follow `scode-build-goal`, "Resource watchdog"
  (Claude Code two-Monitor setup, Codex `skillette-codex-wake-on-exit` setup, or background-sampler + `check` fallback),
  including throwaway `--interval 5` verification before starting the real monitors, stopping any stale prior session's
  monitors on resume, acting immediately on `ALERT`/`WORSE`/`STALE`/`ERROR`, running `watchdog check` before launches
  and at natural boundaries, recording handles/paths in `log.md`, and stopping owned monitors at close
  (`plans/.agents/executing.md`, step 12).

## Decision logging and unattended fallback

- Log consequential design and implementation decisions in `log.md` as they happen with a scannable `DECISION` label and
  the alternatives considered.
- The executing session runs unattended and **never asks the maintainer anything in conversation** — no direct
  questions, no harness question-tool prompt, no approval prompt, no closing "should I...?" or menu of optional next
  steps (`plans/AGENTS.md`, "Unattended means no questions").
- Resolve routine implementation forks within the agreed scope and log them as `DECISION`. Remove unnecessary
  planner-invented machinery when requested behavior and repository constraints still hold.
- When remaining work cannot proceed without a material scope change, weakened guarantee, omitted required behavior, or
  other decision only the maintainer can make, push everything that is consistent and block per
  `plans/.agents/executing.md`, step 10 (`queue block` after a fresh-context cold read). Put things the maintainer
  should know and optional follow-ups into the report rather than stopping or blocking on them.

## Done criterion

The goal is achieved when every PR in the stack exists as a draft PR on GitHub, collectively meets all acceptance
criteria (including removing the covered `TODO.md` entry in PR 2), has passed its Opus 5.5 high review gate, and
satisfies the latest `## Decisions` entry in this plan file (if any). Delivering the report and closing out the claim
happen after the goal is achieved, per `plans/.agents/executing.md`, steps 11–12.

## Release note

No release note needed.

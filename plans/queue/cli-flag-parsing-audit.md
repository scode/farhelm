# CLI flag parsing and full CLI audit

## Goal

Stop the remaining user-facing free-text CLI options and positional arguments in `farhelm` from silently swallowing a
following flag when a value is omitted or mistyped (matching the treatment of `--model` and `--effort` in #5129), and
fix the agent-introduced departures from standard Rust `clap` behavior surfaced during the planning audit.

Covers and removes this `TODO.md` entry in the last code PR:

- Immediate: **Stop other free-text CLI options from swallowing following flags, and audit CLI parsing.**

### Acceptance criteria

- All 25 user-facing identifier/name/path options (`agent rename --session/--expected-title`, `agent stop --session`,
  `agent prompt --session/--prompt-file`, `agent delete --session`, `agent restart --session`,
  `agent create --cwd/--host/--recipe/--container-host/--title/--idempotency-key`,
  `agent clone --source-session/--host/--cwd/--title/--idempotency-key`, `agent host add --alias`,
  `agent host edit --alias`, `TemplateFieldFlags --cwd/--title/--host`, `LaunchFlags --template/--prompt-file`), the 3
  shell/wrapper options (`--wrapper`, `--command`, `--resume-command`), and all 9 positional arguments
  (`agent rename <title>`, `agent template create|edit|delete <name>`, `agent host add <destination>`,
  `agent host edit|remove <host>`, `helm checkout-config set-root <path>`,
  `helm checkout-config set-post-clone <command>`) no longer set `allow_hyphen_values = true`. Omitted values followed
  by a flag (such as `farhelm agent host add dev --alias --tailscale`) fail at parse time; literal hyphen-leading values
  still work via `--flag=-value` or `-- -value`, and bare `--prompt-file -` still reads stdin.
- `--prompt` (on `LaunchFlags` and `AgentCmd::Prompt`), `internal install --expect`, and
  `internal hook --extra-instructions` keep `allow_hyphen_values = true`.
- `--prompt-file` disk and stdin reading runs after `Cli::parse()` (and `hold_own_version`) across all four consumers
  (`spawn`, `agent create`, `agent prompt`, `agent template create|edit`), before dialing the supervisor, so invalid
  flags or conflicts fail immediately without blocking on stdin.
- `LaunchFlags` is split cleanly into `LaunchEditFlags` (12 template/launch edit fields) and `LaunchFlags`
  (`templates` + flattened `LaunchEditFlags`) so `TemplateFieldFlags` no longer flattens `--template` or uses
  `mut_arg("templates", ...)`, while `agent template create|edit --template` keeps its explanatory refusal via a hidden
  refusing field (exit 2) and `spawn --inherit-agent --template` remains refused.
- `farhelm custom-agent report-conversation --help` and `-h` (exact 1-argument form only) print clap help (including
  through the macOS forwarder in `scripts/install.sh`), while all other malformed invocations (`--id=foo`, `--bad-flag`,
  `--id --help`) remain silent exit 0 preserving supervisor `/proc` exact-argv admission; the `--help` carve-out and
  changelog entry are added to `website/src/content/docs/docs/agents/custom-agent-contract.md`.
- Parse-time alignment: `internal install --expect` validation stays out of clap (exit 1, preserving
  `scripts/install.sh`'s exit-2 version check); `Cmd::Spawn` uses a required `.multiple(true)` `ArgGroup` built from the
  13 `LaunchFlags` IDs (excluding `--conversation`, deleting dead `LaunchFlags::any()`); `HostCmd::Edit` keeps its
  existing runtime empty-edit check so agent instructions do not misrepresent combinable flags as mutually exclusive;
  `InternalCmd::AgentScreen::{run_dir, cwd}` validate absolute paths at parse time by sharing a generalized
  `absolute_path` parser with `StopTerminalHosts::state_dir`; `--title-kind` and `--key` use clap's possible-values
  validation locally without adding `clap` to `farhelm-proto`; `spawn` `value_name` labels (`DIR`, `KEY`) align with
  `agent create`; the unused `clap` `"env"` feature is removed from `Cargo.toml`.
- Hidden deprecated tombstone flags (`--profile`, `--profile-id`, `--invocation`, `--mode`, `--confirm-yolo`,
  `agent profiles`) and `farhelm agent help` remain unchanged.
- `SPEC_impl.md`, `crates/farhelm/src/agent_instructions.rs`, tests, and a changelog fragment are updated.

## Decisions already made

- **Hyphen-value scope (maintainer decision after Neon + Opus compatibility review):** Remove
  `allow_hyphen_values = true` from all 25 identifier/name/path options, all 9 positionals, and `--wrapper`,
  `--command`, `--resume-command`. Keep `allow_hyphen_values = true` on `--prompt` (2 sites) so Markdown bullets and
  `---` do not hit clap's misleading option-value tip, and keep it on `internal install --expect` and
  `internal hook --extra-instructions` for upgrade compatibility.
- **`--template` split on template writes (maintainer decision):** Split the structs cleanly AND keep the explanatory
  refusal via a hidden refusing field on `agent template create|edit` (exit 2, matching `--profile`/`--confirm-yolo`).
- **`internal install --expect` and required `ArgGroup`s (maintainer decision):** Leave `internal install --expect`
  validation out of clap (exit 1) so `scripts/install.sh` never misdiagnoses a bad expectation as "release older than
  this installer"; build `Cmd::Spawn`'s required group with `.multiple(true)` excluding `--conversation`; keep
  `HostCmd::Edit`'s runtime check rather than adding multi-select group notation to `agent_instructions.rs`.
- **`custom-agent report-conversation --help` (maintainer decision):** Restore exact 1-argument `--help`/`-h` only (all
  other malformed args stay silent exit 0) and document the carve-out in
  `website/src/content/docs/docs/agents/custom-agent-contract.md`.
- **Other audit cleanups (maintainer decision):** Move `--prompt-file` I/O out of `value_parser` after parsing; move
  `InternalCmd::AgentScreen` absolute-path check to parse time; use possible-values parsing for `--title-kind` and
  `--key`; align `spawn`/`create` `value_name` drift; drop unused `clap` `"env"` feature; keep hidden tombstones and
  `farhelm agent help` as-is.
- **Review gate (maintainer decision):** Single fresh-context agent on **Opus 5.5** at **high** reasoning effort with
  the general review charter below.

## Implementation outline and requirement sources

### Requirement sources

- **User request and decisions:** All items under "Decisions already made" above; no release note highlight needed.
- **Binding repository constraints:** `SPEC_impl.md` (CLI parsing notes around lines 7806 and 7852–7864 — update
  explicitly to match the new contract rather than leaving stale spec text),
  `website/src/content/docs/docs/agents/custom-agent-contract.md` (contract carve-out + contract changelog entry),
  `scripts/install.sh` (exit-2 meaning on `internal install`, macOS forwarder silencing on
  `custom-agent report-conversation`), `crates/farhelm-supervisor/src/procs/positional.rs` (exact 5-element argv for
  `custom-agent report-conversation --id <val>`), `AGENTS.md` ("Changelog fragments", "Tests and validation"),
  `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`.
- **Planner-proposed implementation choices (reviewed by Neon + Opus compatibility pair and fresh-context planning
  review):**
  - Use a clean 2-struct split (`LaunchEditFlags` + `LaunchFlags` placing `templates` directly before `edits`, updating
    the one pinned usage-order assertion in `crates/farhelm/src/agent_instructions.rs:893-898`) rather than an
    artificial 3-struct split solely to preserve `--template`'s old mid-struct position.
  - Narrow `LAUNCH_FLAG_IDS` in `crates/farhelm/src/main.rs` to the 13 `LaunchFlags` IDs, declare
    `conflicts_with = "conversation"` directly on `Cmd::Spawn::inherit_agent`, share `LAUNCH_FLAG_IDS` with
    `ArgGroup::new("spawn_launch").required(true).multiple(true).arg("inherit_agent").args(LAUNCH_FLAG_IDS)`, and delete
    dead `LaunchFlags::any()` and its unreachable `else { bail!(...) }`.
  - Generalize `absolute_state_dir` in `crates/farhelm/src/main.rs` into one shared `absolute_path` parser used by
    `StopTerminalHosts::state_dir` and `AgentScreen::{run_dir, cwd}` (keeping `run_dir_fits` at runtime).
  - Replace `parse_template_key` with clap's `value_parser = TEMPLATE_KEYS` directly (`["1", ..., "0"]`) and replace
    `parse_template_title_kind` with a local `PossibleValuesParser::new(["base", "fixed"]).map(...)` following
    `host_icon_parser()` (no `clap` dependency added to `farhelm-proto`). Keep `value_name = "base|fixed"` and
    `value_name = "DIGIT"`.
  - Keep `prompt_file: Option<PathBuf>` (handling non-UTF-8 paths while bare `-` still works without
    `allow_hyphen_values`) and run a single `resolve_prompt_files(&mut cli.command)?` pass right after
    `hold_own_version(&cli.command)?`, reusing `read_initial_prompt` / `read_agent_prompt` and adding
    `debug_assert!(self.prompt_file.is_none())` in `edits()`/`verb()`. Update the two `agent_cli.rs` exit-2 assertions
    for `--prompt-file` content errors (now exit 1, still failing before dialing).
  - Add the matching exact 1-argument `--help`/`-h` carve-out in `scripts/install.sh` (around line 352, excluding
    `$# -eq 3` with `$3` equal to `--help` or `-h`) and cover it in `scripts/test-install-sh.sh` so macOS installs also
    print help.
  - Rewrite the existing hyphen-leading value tests (`crates/farhelm/tests/agent_cli.rs`, `crates/farhelm/src/main.rs`)
    to pass the same hyphen-leading values via `=` or `--` (preserving wire-delivery coverage) and add negative tests
    proving omitted values followed by flags are refused. Add a concise note in
    `crates/farhelm/src/agent_instructions.rs` explaining `=` and `--` for hyphen-leading values.

### PR shape (2 PRs)

1. **`plan/cli-flag-parsing-audit/01-remove-permissive-hyphen-values`:** Remove `allow_hyphen_values = true` across the
   37 target options/positionals in `crates/farhelm/src/main.rs`, update doc comments, `agent_instructions.rs`,
   `SPEC_impl.md`, existing hyphen-value tests (switching to `=`/`--`), add flag-swallowing regression tests, and add a
   changelog fragment (`kind: changed`).
2. **`plan/cli-flag-parsing-audit/02-clap-audit-cleanups`:** Post-parse `--prompt-file` I/O across all 4 consumers,
   `LaunchEditFlags`/`LaunchFlags` 2-struct split + hidden refusing `--template` on template writes, exact 1-arg
   `--help`/`-h` on `custom-agent report-conversation` (in `main.rs`, `scripts/install.sh`,
   `scripts/test-install-sh.sh`, and `custom-agent-contract.md`), `Cmd::Spawn` required `ArgGroup` (keeping `host edit`
   and `install --expect` at runtime), shared `absolute_path` parser on `AgentScreen`, possible-values parsers on
   `--title-kind`/`--key`, `value_name` alignment, drop unused `clap` `"env"` feature in `Cargo.toml`, update
   tests/spec/fragment, and remove the `TODO.md` entry.

## Scope reassessment

Before implementing a substantial departure from the outline above, run a fresh-context review at the executing
session's own model and effort with the following charter verbatim. Supply the original request, settled decisions,
outline, current diff, and proposed departure (what changed, why it is necessary, and which simpler alternative was
ruled out). A close reuse-versus-duplication call not foreseen during planning is a routine fork when every option stays
within the agreed scope: decide after reassessment and log as `DECISION`. Repeated corrective reviews of the same
component also require reassessing necessity before another repair round. Give any same-model sub-agent bounded units
within the outline; unapproved new mechanisms are outside its task.

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

## The checkout

This goal runs inside the checkout the executing session runs in, identified as the nearest Git worktree or jj workspace
containing the session's working directory whose remotes include `scode/farhelm` (comparing host, owner, and name,
ignoring `.git` suffix and SSH vs. HTTPS). Every repository path in this file is relative to that checkout's root.
Confirm this first; if the nearest checkout is not `scode/farhelm`, stop before the resume protocol, write nothing
(including the log), and report that the goal must be run from a checkout of `scode/farhelm`.

## Resume protocol

Right after confirming the checkout, resolve the plan's working log directory with
`cargo xtask records path plan/cli-flag-parsing-audit` (`plans/.agents/outside-files.md`, "Files outside the
repository"). Its `log.md` is the working log. Invoke the `agent-resumeable` skill with that resolved path: if `log.md`
already exists, read it in full and resume where the previous holder left off (cross-checking against VCS state and open
`plan/cli-flag-parsing-audit/*` PRs on GitHub); if it does not exist, this is a fresh start.

## Execution mode

Run in **no-workhorse** mode. Do not send any unit of the executing agent's own decomposition to a weaker model or lower
reasoning effort than the executing session's own. Do not invoke, load, or follow `scode-galaxy-brain` or
`scode-model-routing` at any point; if `scode-galaxy-brain` is already active, this goal file is the maintainer's
express instruction to stop it.

Same-model sub-agents at the executing session's own model and effort are allowed when they are likely to finish work
sooner without costing quality; concurrent writers may use isolated worktrees or jj workspaces that the executing agent
merges back, while the executing agent owns the `jjstack` stack and every PR. The fresh-context read-only sub-agents
required by `plans/AGENTS.md`, "Sub-agents this file requires" (resume check, cold reads of Blocked questions and
reports), the scope reassessment review (at the executing session's own model and effort), and the per-PR review gate
below are explicitly required and exempt from any no-delegation restriction. Launch any sub-agent whose demanded model
is not available natively through `$scode-harness-shellout`.

## Resource watchdog

Mandatory before starting implementation or delegations. Resolve the `scode-build-goal` skill's `scripts/watchdog` path
via `realpath` at run time. Put the watchdog status file at `farhelm-plan-cli-flag-parsing-audit-watchdog.json` in the
parent directory of the executing checkout (derived per `scode-build-goal`, "Placing the files") — never inside the
`plan/cli-flag-parsing-audit` record directory.

- **Watched paths (`--path`):** the executing checkout root, `$HOME`, `/tmp`, the directory holding the status file, the
  executing session's scratch directory, and any worktree/build directory on another filesystem (restart the sampler if
  added). Interval: 60 seconds; default alert levels.
- **Harness setup & verification:** Before starting the real monitors, verify the delivery mechanism once per session on
  throwaway status files in the scratch directory with `--interval 5` (`--synthetic-alert`, killing via status `pid`,
  and `--stall-after 2` watched by an observer armed first). On Claude Code, run `observe` and `sample` under separate
  Monitors (30-minute timeout, observer first); on Codex, arm `observe --exit-on-event` then `sample` through
  `skillette-codex-wake-on-exit`; otherwise log the fallback, run the sampler in the background, and run
  `watchdog check --status <file>` before every launch and at natural boundaries.
- **Acting on shortages/failures & lifecycle:** On `ALERT`/`WORSE` or failing `check`, pause launches, clean run-owned
  build leftovers/worktrees, and resume only once `check` passes (if nothing run-owned can free enough, block/unclaim
  per `plans/.agents/executing.md`). Record monitor handles, watched paths, and delivery mechanism in the working log;
  on resume, stop any still-live prior monitors before starting new ones; stop owned monitors when the plan closes.

## Stack and PR discipline

Build a linear stack of draft PRs using the `jjstack` skill on top of `main@origin` (or this plan's own open
`plan/cli-flag-parsing-audit/*` PRs when resuming, never another plan's PRs), set up per `plans/.agents/executing.md`,
"Executing one plan". Use bookmark names `plan/cli-flag-parsing-audit/<nn>-<short-name>`. Keep every PR a draft — never
mark a PR ready and never merge. Restructure existing PRs in the stack instead of stacking fixup commits on top,
including PRs created by an earlier run. Never edit anything under `plans/`.

Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments", and "Tests and validation", plus
`.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`. The last code PR removes the
covered `TODO.md` entry.

## Review gate

Before finishing any PR in this stack, delegate an independent fresh-context review to a single agent on **Opus 5.5** at
**high** reasoning effort (explicit model and effort demand; launch natively if supported or via
`$scode-harness-shellout` without hardcoding a CLI command here). Give the reviewer no conversation context from the
executing session; supply the repo root of the executing checkout, the commit range or bookmark to review, the private
findings file path in the executing session's scratch directory, `.agents/test-authoring.md` verbatim when tests or
fixtures changed, and this charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved. Report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

Address all valid findings and re-verify before moving on.

## Decision logging

Log major design decisions in the plan's working log as they happen with a scannable `DECISION` label, noting the
alternatives considered.

## Unattended fallback

This plan runs unattended. Never ask the maintainer anything in conversation — not a question, not an approval prompt,
not an optional offer, and not a harness question tool prompt (`plans/AGENTS.md`, "Unattended means no questions").
Resolve routine implementation forks within the agreed scope, log consequential choices as `DECISION`, and carry on. If
a required decision goes beyond the agreed scope, weakens a guarantee, or hits an unresolvable conflict, block per
`plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Put non-blocking notes and possible follow-ups
in the closing report.

## Done criterion

Every PR in the stack exists on GitHub as a draft, has passed its Opus 5.5 high review gate and applicable validation,
removes the covered `TODO.md` entry in the last code PR, and satisfies the latest `## Decisions` entry (if any).
Delivering the report is the executor's closing step (`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.

# Support Antigravity

Written against main at c13d780a on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Support Antigravity.** add support for antigravity.

The maintainer's note from the planning page, verbatim:

> make sure adding the Antigravity section to the doc (with hook installation instructions) is part of the plan

### Acceptance criteria

1. **Structured launches (`agy`).** Users can pick **Antigravity** on the session launcher's Agent tab (chip label
   `"Antigravity"`, placed directly after Claude in `HARNESS_PICKER_ORDER` and `HARNESS_SEARCH_ORDER`). Farhelm compiles
   its command with binary `agy`:
   - Model (`--model <id>`): catalog suggests Antigravity's Gemini models only — `gemini-4-neon` (family `neon`, efforts
     `low`, `medium`, `high`, `xhigh`), `gemini-4-argon` (family `argon`, efforts `low`, `medium`, `high`),
     `gemini-4-hydrogen` (family `hydrogen`, efforts `low`, `medium`, `high`, `xhigh`), and `gemini-3.8-flash` (family
     `flash`, efforts `low`, `medium`, `high`), all marked `latest: true`. Custom model IDs remain accepted, with
     `harness_efforts` offering `[Low, Medium, High, Xhigh]`.
   - Effort (`--effort <level>`): passed when chosen.
   - Permissions: **Guarded** (`--mode auto`, the default when omitted on new requests), **YOLO**
     (`--dangerously-skip-permissions`), and **Custom** permission flags. Historical omitted permissions keep `None`
     (`stored_permission`). Workspace trust has no per-launch flag (`offers_workspace_trust` is `false`).
   - Initial prompt: compiled via `--prompt-interactive <prompt>` (after verifying against installed `agy` whether
     `--prompt-interactive` combines with `--conversation <id>`, setting `supports_initial_prompt(named)` accordingly).
   - Named conversation start and Restart resume: `--conversation <id>` (not a file path; `named_conversation_is_file`
     is `false`, `captures_named_conversation` is `true`).
   - `farhelm agent create --agent antigravity` works through the existing `LaunchHarness::ALL` path.
2. **User-installed hook conversation tracking and Restart.**
   - `AgentKind::Antigravity` and `ReportVendor::Antigravity` exist in `farhelm-proto`, bumping `PROTOCOL_VERSION` once
     for the stack.
   - `farhelm internal hook --vendor antigravity` reads Antigravity's camelCase hook JSON on stdin (`conversationId`,
     `hookEventName`, optional `vendor`), allowlists `"SessionStart"` and `"Stop"`, rejects subagent markers if `agy`
     emits any (verified against installed `agy`), records process ancestry, writes the drop-file report when Farhelm
     session credentials are present, always exits `0`, prints nothing to stderr, and **unconditionally writes `{}\n` to
     stdout** after `hook::run_with` returns on every exit path (including outside Farhelm / `no-credential`,
     `bad-payload`, `timeout`, `write-failed`, and caught panic), ignoring broken-pipe errors and never appending
     plain-text `POINTER_LINE` even if `--announce` is passed.
   - The supervisor's `AntigravityIntegration` does not inject per-launch hook flags (`farhelm_args` returns
     `FarhelmArgs::none()`, `hook_argv` returns `Vec::new()`, `inject_hooks` returns silent unhooked `HookInjection`),
     so the 65-second silent-hook tripwire never fires when the user has not installed hooks, and
     `AgentHooks::silences(AgentKind::Antigravity)` is `false` (not added to `parse_agent_hooks`). No `$farhelm`
     instructions pointer is announced on user-installed hooks.
   - Conversation admission reuses `procs::positional_corridor` and `Supervisor::report_positional_conversation` with an
     `ANTIGRAVITY` `PositionalReportPolicy` (`quiet_event: "Stop"`, `NotificationKind::AntigravityPinCorrected`).
   - `AgentKind::restart_readiness` and `RestartReadiness::clause` no longer hardcode `"Grok"` for user-installed hooks:
     both Grok and Antigravity name their own hooks/docs accurately (and reflect the verified timing of `agy`'s
     `SessionStart`, whether at startup or after first prompt).
   - Screen status uses `GenericReader` (basic working/idle; no live-captured TUI screen reader or `status_line_yolo`
     reader in this plan).
3. **Desktop GUI mark, color, wording, and test harness.**
   - `HarnessGlyph::Antigravity` draws the approved Farhelm original **"Rising chevron & bar"** mark in the shared
     12-unit normalized box (upward chevron above a short horizontal bar; never copying or tracing Google's logo), with
     its generated SVG `crates/gui/farhelm-gui/assets/icons/harness-antigravity.svg` matching
     `every_harness_icon_is_its_mark_rendered`.
   - Identity color is rose-magenta: `#e85d9a` (dark) / `#c73c7a` (light) in `Palette::harness` (`[Token; 8]`), verified
     to clear 3:1 contrast against both sidebar and row hover surfaces in both themes.
   - `docs/harness-marks.md` records the mark's Farhelm-original provenance (citing Google's third-party brand terms),
     bounding box, and color/contrast record.
   - Launcher chip, search, permission help (`Guarded · Antigravity auto mode`), conversation selector label
     (`"Conversation ID"`), `agent_note` (`None`), and `settings/agent_trust.rs` (`None`) are wired exhaustively.
   - `scripts/gui-agent.py` adds `"agy"` to `STAGED_AGENTS` and `"antigravity"` to `LAUNCH_HARNESSES`, and affected GUI
     goldens (`dialog-launcher-frame`, `template-launcher`, etc.) are updated and visually checked for chip-row
     wrapping.
4. **Specs and user documentation.**
   - `SPEC.md` and `SPEC_impl.md` document Antigravity's launch choices, permission flags, user-installed hook contract,
     positional attribution, and conversation tracking.
   - Add `website/src/content/docs/docs/agents/antigravity.md` covering setup, exact `~/.gemini/config/hooks.json`
     installation instructions (calling `<absolute-path-to-farhelm> internal hook --vendor antigravity` on
     `SessionStart` and `Stop`), Restart behavior, and gotchas (including basic status and any verified subagent or
     wrapper caveats).
   - Update `website/src/content/docs/docs/agents/index.mdx` (supported-agents matrix row with `warn: 'n'`, Custom
     agents list, Guarded table, and caveats), `website/src/content/docs/docs/using/stop-restart-resume.mdx`,
     `website/src/content/docs/docs/agents/custom-commands.md`,
     `website/src/content/docs/docs/agents/agent-wrappers.md`,
     `website/src/content/docs/docs/how-it-works/agent-hook-injection.md`, and (if `--prompt-interactive` cannot combine
     with `--conversation`) `website/src/content/docs/docs/using/start-a-session.mdx`.
5. **Tests, changelog fragment, and TODO removal.**
   - Unit and e2e tests cover Antigravity launch compilation, named start, `hook.rs` payload parsing and unconditional
     `{}\n` stdout on all exit paths, positional pin attribution and correction (`claude_pin.rs`), end-to-end hook
     reporting and Restart (`hook_identity.rs` with a minimal fake-agent fixture + `structured_launches.rs`), GUI
     mark/color/launcher/search/row tests, and `scripts/test-gui-agent.py`.
   - PR 1 includes a changelog fragment under `releasing/changelog.d/` (`python3 releasing/check-changelog.py format`),
     and the last code/docs PR removes this plan's entry from `TODO.md`.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page (which showed SVG mark mockups, color swatches, and a
launcher chip strip):

- **Scope:** Launch + user-installed hook tracking (basic status; live TUI screen reader deferred to a later follow-up).
- **Antigravity docs section required:** Maintainer explicitly asked to include the Antigravity docs page with hook
  installation instructions in this plan.
- **Permission modes:** Guarded (`--mode auto`), YOLO (`--dangerously-skip-permissions`), Custom.
- **Model catalog:** Gemini models only (`gemini-4-neon`, `gemini-4-argon`, `gemini-4-hydrogen`, `gemini-3.8-flash`),
  with base model IDs paired with `--effort`.
- **Sidebar mark:** Farhelm original **"A. Rising chevron & bar"** (12-unit normalized box).
- **Sidebar mark color:** **Rose-magenta** (`#e85d9a` dark / `#c73c7a` light).
- **Launcher chip:** Label `"Antigravity"`, placed directly after Claude.
- **Review gate:** Single fresh-context `gpt-6.2-sol` agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Release note:** Maintainer supplied custom release note wording (reproduced verbatim in `## Release note` below).
- **Scheduled as `[pending]`**, with no dependency on other queued plans.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the planning page note, and the Decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Code rules": every per-harness decision lives in the named exhaustive places mapped in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` (no `kind == X`, `matches!` over specific harnesses, or `_` arms in
  shared code).
- `docs/harness-marks.md`: 12-unit box with 10-unit span normalization rule, generated SVG files matching `marks.rs`
  (`every_harness_icon_is_its_mark_rendered`), 3:1 contrast in both themes, provenance documented.
- `crates/farhelm-proto/src/lib.rs`: `PROTOCOL_VERSION` must bump on any new wire enum variant (`AgentKind`,
  `LaunchHarness`). Never land two commits on `main` that add wire variants under the same protocol version.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and
  `.agents/test-authoring.md`.
- `website/AGENTS.md` for docs changes (`bun run build` validates all internal links and anchors).

**Planner proposals** (verified against the repository and reviewed by an independent fresh-context planning reviewer):

- **Reuse positional attribution (`procs/positional.rs` + `service/core/vendor/positional.rs`)** instead of building a
  Grok-style custom corridor/locator: `agy` reports plain conversation UUIDs on `SessionStart` and `Stop` via `sh -c`,
  with no shared leader daemon (`--no-leader`) and no two-file record-pair verification.
- **Inline `AntigravityIntegration`** (~15 lines) in `crates/farhelm-supervisor/src/agent_kind/mod.rs` beside
  `GrokIntegration`, instead of adding a new `agent_kind/antigravity.rs` file.
- **Keep `parse_antigravity_payload` (`crates/farhelm/src/hook.rs`) and `NotificationKind::AntigravityPinCorrected`
  (`notifications.rs`) dedicated** rather than over-generalizing snake_case parsers or SQLite notification keys.
- **Generalize `RestartReadiness::FirstPromptWithHooks`** (in `crates/farhelm-proto/src/lib.rs`) so it no longer
  hardcodes `"Grok"` across its five callers — either agent-neutral wording
  (`"after your first prompt, and only if Farhelm's hooks for it are installed (see its page in Farhelm's documentation)"`,
  keeping `&'static str`) or a small `clause_for(self, name: &str, wording: ReadinessWording)` helper if naming the
  agent page directly reads better.
- **Two-PR stack** (one code PR + one docs/specs/TODO PR) so `AgentKind::Antigravity` and `LaunchHarness::Antigravity`
  land together under a single `PROTOCOL_VERSION` bump with zero half-wired intermediate GUI state and real-launch e2e
  tests.
- **Verify four `agy` CLI facts locally before coding** (from `agy` on the executing host if installed, or `agy` docs;
  log each finding as a `DECISION`):
  1. Whether `agy`'s `SessionStart` hook fires immediately at startup or on the first prompt (governs `RestartReadiness`
     timing wording).
  2. Whether `agy` fires `SessionStart`/`Stop` for subagents and whether a subagent marker field appears in stdin JSON
     (if so, map it into `report.agent_id` in `parse_antigravity_payload` so shared `subagent_refusal` rejects it;
     otherwise document the caveat in `antigravity.md`).
  3. Whether `agy --mode auto --conversation <id> --prompt-interactive <prompt>` works together (governs
     `supports_initial_prompt(named)` and whether `start-a-session.mdx` notes a restriction).
  4. What `/proc/<pid>/exe` basename `agy` runs under (observed as native binary `agy` at planning time ->
     `executable_basename(AgentKind::Antigravity)` = `Some("agy")`, or `None` if wrapper-launched on a target platform).

## Outline

A two-PR linear stack on `main@origin`:

1. **`plan/antigravity-support/01-antigravity-launch-and-hooks` (`feat:`):**
   - `crates/farhelm-proto/src/lib.rs` & `launch.rs`: Add `AgentKind::Antigravity`, `ReportVendor::Antigravity`,
     `LaunchHarness::Antigravity`, bump `PROTOCOL_VERSION` (and pin test), add `display_name`, `word`, generalized
     `RestartReadiness` wording, and all exhaustive `LaunchHarness` methods (`agent_kind`, `pre_launch_agent_kind`,
     `offers_model`, `catalog_model_id`, `offers_effort`, `offers_permission`, `omitted_permission`,
     `stored_permission`, `guarded_flags`, `supports_initial_prompt`, `initial_prompt_arguments`,
     `offers_named_conversation`, `named_conversation_is_file`, `captures_named_conversation`,
     `offers_workspace_trust`).
   - `crates/farhelm-helm/src/launches.rs` & `checkout_config.rs`: Add `agy` binary, Gemini catalog entries
     (`gemini-4-neon`, `gemini-4-argon`, `gemini-4-hydrogen`, `gemini-3.8-flash`), `harness_efforts`
     (`[Low, Medium, High, Xhigh]`), argv compiler (`--model`, `--effort`, `--mode auto`,
     `--dangerously-skip-permissions`, Custom, `--conversation {conversation}`), `named_start`, and `workspace_trust_*`
     (`false` / `&[]`).
   - `crates/farhelm-supervisor/`: Inline `AntigravityIntegration` in `agent_kind/mod.rs` (`FarhelmArgs::none()`,
     `AgentHooks::silences` = `false`, `prompt_delivery` = `None`, `executable_basename`,
     `accepts_reported_conversation`, `conversation_report_slot`, `hook_command`, etc.); `GenericReader` +
     `LaunchHarness` arms in `screen_reader.rs`; `store.rs` `"antigravity"` column mapping; `pi_extension.rs`;
     `service/core.rs` + `vendor/positional.rs` (`ANTIGRAVITY` policy); `service/notifications.rs`
     (`NotificationKind::AntigravityPinCorrected`, `ALL` array length).
   - `crates/farhelm/src/hook.rs` & `main.rs`: Add `HookVendor::Antigravity`, `parse_antigravity_payload` (camelCase
     `conversationId`, allowlisted `SessionStart` / `Stop`, optional `vendor`), vendor-aware stdout helper emitting
     unconditional `{}\n` after `run_with` on every exit path and ignoring `--announce`, and update `hook.rs` module doc
     rule #1.
   - `crates/gui/farhelm-gui/`: Add `HarnessGlyph::Antigravity` ("Rising chevron & bar" in `marks.rs`), regenerate
     `assets/icons/harness-antigravity.svg` via `write_harness_mark_svgs`, add `Icon::HarnessAntigravity` (`assets.rs`),
     add rose-magenta (`#e85d9a` / `#c73c7a`) to `Palette::harness` (`[Token; 8]` in `theme.rs`), wire `harness.rs`,
     `launcher/words.rs` (`HARNESS_PICKER_ORDER`, `HARNESS_SEARCH_ORDER`, `agent_note` = `None`), `launcher/draft.rs`
     (`harness_label`), `launcher/view.rs` (`conversation_label`), `permission/words.rs` (`guarded_mode`),
     `settings/agent_trust.rs` (`None`), and `terminal/drag_notice.rs`. Update `docs/harness-marks.md` and affected GUI
     goldens (`dialog-launcher-frame`, `template-launcher`, checking chip-row wrapping visually).
   - `scripts/gui-agent.py`: Add `"agy"` to `STAGED_AGENTS` and `"antigravity"` to `LAUNCH_HARNESSES`.
   - Tests & fixtures: Minimal Antigravity hook script in `crates/farhelm-fixtures/src/fake_agent.rs` (asserting exit 0
     and `{}\n` stdout); unit tests in `farhelm-proto`, `farhelm-helm/src/launches.rs`, `farhelm/src/hook.rs`,
     `farhelm-supervisor` (`claude_pin.rs`, `agent_kind/mod.rs`), `farhelm-gui` (`harness.rs`, `launcher/*_tests.rs`,
     `session_list/row_marks_tests.rs`); e2e tests in `hook_identity.rs` and `structured_launches.rs`;
     `scripts/test-gui-agent.py`.
   - Changelog fragment under `releasing/changelog.d/`.

2. **`plan/antigravity-support/02-antigravity-docs-and-specs` (`docs:`):**
   - Update `SPEC.md` and `SPEC_impl.md` for Antigravity launches, permissions, user-installed hooks, and conversation
     tracking.
   - Add `website/src/content/docs/docs/agents/antigravity.md` (setup, exact `~/.gemini/config/hooks.json` snippet for
     `SessionStart` and `Stop`, Restart behavior, gotchas).
   - Update `website/src/content/docs/docs/agents/index.mdx` (matrix row with `warn: 'n'`, Custom list, Guarded table,
     caveats), `website/src/content/docs/docs/using/stop-restart-resume.mdx`,
     `website/src/content/docs/docs/agents/custom-commands.md`,
     `website/src/content/docs/docs/agents/agent-wrappers.md`,
     `website/src/content/docs/docs/how-it-works/agent-hook-injection.md`, and (if needed)
     `website/src/content/docs/docs/using/start-a-session.mdx`.
   - Remove the **Support Antigravity** entry from `TODO.md`.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused, and skipped:

- PR 1: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, targeted `cargo nextest run` selections through
  `python3 scripts/record-test-run.py` covering `farhelm-proto`, `farhelm-helm` (`launches::`), `farhelm-supervisor`
  (`agent_kind::`, `claude_pin::`, `notifications::`, `store::`), `farhelm` (`hook::`, `e2e` `hook_identity::` and
  `structured_launches::`), and `farhelm-gui` (`harness::`, `marks::`, `launcher::`, `row_marks::`),
  `python3 -B scripts/test-gui-agent.py`, `python3 scripts/check-gui-goldens.py` (through the recorder; publish updated
  goldens if visual diffs are intended), `python -B scripts/check-test-sleeps.py` (if Rust tests changed), and
  `python3 releasing/check-changelog.py format`.
- PR 2: `dprint check` on changed Markdown files, `cd website && bun install --frozen-lockfile && bun run build`, and
  `python3 scripts/check-agents-md.py` (if any heading cited by instruction files changed). No Rust tests needed for PR
  2's docs-only diff.

## Fallbacks

- If `agy` is not installed on the executing machine, verify the four CLI/hook questions from repository/installed docs
  where possible, use the verified `agy` 1.5.12 facts recorded in this plan (`SessionStart`/`Stop` camelCase
  `conversationId` with `{}\n` stdout, `FirstPromptWithHooks`, `supports_initial_prompt(true)`, `Some("agy")`), note any
  unverified runtime assumption in the report's "things you should know", and do not block.
- If `agy`'s `SessionStart` fires immediately at process startup (rather than on first prompt), add a
  `ShortlyAfterStartWithHooks` variant to `RestartReadiness` instead of misstating timing with `FirstPromptWithHooks`,
  and log it as a `DECISION`.
- If `agy` cannot combine `--prompt-interactive` with `--conversation`, return `!named` from `supports_initial_prompt`
  and add Antigravity beside Muse in `website/src/content/docs/docs/using/start-a-session.mdx`.
- If updating GUI goldens is blocked because the executing machine lacks push access to the private golden storage repo,
  keep the updated captures verified locally and block per step 10 rather than landing failing goldens.
- If another queued plan bumps `PROTOCOL_VERSION` before this plan lands, rebase cleanly onto the latest version and
  increment by 1 from `main@origin` (`.agents/careful-rebase.md`).

Decisions to log: the four `agy` CLI verification outcomes, the `RestartReadiness` generalization shape chosen, the
exact normalized bounding box and contrast ratios for the rose-magenta mark, any golden scenes updated (and launcher
chip-row wrapping inspection), and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-antigravity-support-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-antigravity-support-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation".

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/antigravity-support/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's entry for the release, not an instruction to
  you: each PR still leaves the fragment its type calls for, describing what that PR built, and where what got built
  departs from the release note (a renamed control, a part left out, a limitation it does not mention), the fragment's
  context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

Basic support for Antigravity (agy) is now available. It supports launching and session tracking - but session tracking
requires manual installation of Antigravity hooks (see the Antigravity section of the documentation).

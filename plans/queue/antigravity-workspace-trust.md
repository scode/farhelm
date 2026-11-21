# Antigravity workspace trust and discovery hint

Written against main at 7d4d49b6 on 2026-11-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Antigravity workspace trust and discovery hint.** Add an off-by-default Antigravity switch under Settings → Agent
> trust that suppresses Antigravity's "trust this directory?" prompt on all Antigravity launches (managed checkouts and
> plain folders alike) via Antigravity's per-launch `--settings` flag (`{"trustedWorkspaces":["<canonical-cwd>"]}`)
> without editing Antigravity's saved `settings.json`, omitting Farhelm's `--settings` when Custom permission flags
> already pass `--settings`. Recognize Antigravity's trust prompt on screen so the session reports Waiting while it is
> up and shows the non-modal discovery banner above the terminal when the switch is off. Plan:
> `plans/queue/antigravity-workspace-trust.md`.

### Where it stands (from planning, at main 7d4d49b6)

Verified empirically against Antigravity CLI (`agy`) 1.5.13 in fresh scratch directories on Linux:

- `agy` has no dedicated `--trust-workspace` flag, and `--dangerously-skip-permissions` (YOLO) still shows the workspace
  trust prompt in an untrusted folder.
- Passing `--settings '{"trustedWorkspaces":["<canonical-cwd>"]}'` suppresses the prompt and goes straight to the input
  box, both alone and combined with `--mode auto` (Guarded) or `--dangerously-skip-permissions` (YOLO), without writing
  to `~/.gemini/antigravity-cli/settings.json` (verified unchanged) and while preserving other keys from `settings.json`
  (such as `model`).
- `trustedWorkspaces` matches the exact symlink-resolved canonical path of the working directory: trusting a parent
  directory does not trust subdirectories, and when launched inside a symlinked directory, passing the symlink path
  still prompts while passing the canonical path skips the prompt.
- Two vendor caveats verified on `agy 1.5.13`:
  1. Passing `trustedWorkspaces` via `--settings` replaces the saved `trustedWorkspaces` array for that session rather
     than appending to it.
  2. Passing `--settings` twice is last-wins: the second `--settings` completely replaces the first.
- The trust dialog `agy 1.5.13` draws on screen:
  - `Accessing workspace:` followed by the directory path
  - `Do you trust the contents of this project?`
  - `Antigravity CLI requires permission to read, edit, and execute files here.`
  - `> Yes, I trust this folder` / `No, exit`
  - Navigation footer `↑/↓ Navigate · enter Confirm` (followed on the same line by model/effort text in the status row).
- In Farhelm today:
  - `LaunchHarness::Antigravity` returns `false` from `offers_workspace_trust()` (`crates/farhelm-proto/src/launch.rs`)
    and has no entry in `TrustAgent` (`crates/gui/farhelm-gui/src/settings/agent_trust.rs`), `checkout_config`, or
    `preferences`.
  - `AgentKind::Antigravity` maps to `&GenericReader` in `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`
    (generic change-counting Working/Idle only, no trust-prompt recognition).
  - Codex already establishes the per-launch placeholder pattern (`CODEX_TRUSTED_CWD_PLACEHOLDER` in
    `crates/farhelm-proto/src/session_launch.rs`, substituted to the canonical `cwd` by `fill_cwd` in
    `crates/farhelm-supervisor/src/agent_kind/mod.rs`), and schema 57 established the helm DB +
    `/api/checkout-agent-trust/{harness}`
    - `TrustAgent` + `TrustBand` pattern for Codex, Muse and Pi.

### Acceptance criteria

1. **Per-launch Antigravity trust placeholder and supervisor substitution.**
   - `farhelm-proto` defines `ANTIGRAVITY_TRUSTED_CWD_PLACEHOLDER` (`"{antigravity:trusted-cwd}"`) in
     `session_launch.rs`, includes it in `RESERVED_PLACEHOLDERS`, flips `LaunchHarness::Antigravity` to `true` in
     `LaunchHarness::offers_workspace_trust()`, adds `antigravity_trust_hint_dismissed: Option<bool>` to `Preferences`
     and `PreferencePatch`, and bumps `PROTOCOL_VERSION` (+1) so older supervisors that would pass the placeholder
     literally cannot connect.
   - `farhelm-supervisor`'s `fill_cwd` and `has_cwd_placeholder` (`agent_kind/mod.rs`) resolve `cwd` with
     `std::fs::canonicalize` (falling back to `cwd` if canonicalization fails, reusing the Codex canonicalization path)
     and replace `ANTIGRAVITY_TRUSTED_CWD_PLACEHOLDER` with a valid JSON object `{"trustedWorkspaces":["<resolved>"]}`
     (encoded with `serde_json` so any path characters are escaped safely).
2. **Antigravity trust screen reader, Waiting status, and captured fixture.**
   - `AntigravityReader` in `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs` (wired in
     `reader_for(AgentKind::Antigravity)`) recognizes Antigravity's `"Do you trust the contents of this project?"`
     dialog in `trust_prompt(&self, text)` (normalized across wrapped lines, anchored on the dialog's prompt,
     explanation, both menu choices, and navigation footer so quoted phrases in a transcript or completed output cannot
     false-positive).
   - `AntigravityReader::read` returns `Reading::anchored(ScreenState::Waiting)` while `trust_prompt` is true, and falls
     back to `generic_reading(counts)` otherwise. `AntigravityReader::has_screen_rules()` stays `false` so normal
     Antigravity screens never trigger fallback warnings in `ticker.rs`.
   - `workspace_trust_prompt` (`screen_reader.rs`) reports `true` for Antigravity launches while the trust dialog is on
     screen and clears when it leaves or capture fails.
   - A real, sanitized Antigravity `1.5.13` capture is committed under
     `crates/farhelm-supervisor/tests/fixtures/screens/antigravity/1.5.13/waiting-trust.{txt,title}` (plus a narrow
     derived fixture only if real Antigravity actually clips or alters wording at narrow terminal widths), obeying
     `docs/agent-screen-fixtures.md` and passing `screen_fixtures_carry_no_personal_data`. `screen_fixtures.rs` is
     updated to handle trust-only harnesses without forcing an unimplemented `working`/`idle` activity contract, and
     inline unit tests in `screen_reader.rs` cover negative cases (idle prompt, quoted trust text, post-dialog output,
     wrapped lines).
3. **Helm storage, API, all-launches trust scope, and Custom `--settings` omission.**
   - Next helm DB schema migration in `crates/farhelm-helm/src/store.rs` adds
     `antigravity_trust INTEGER NOT NULL DEFAULT 0` to `checkout_config` and `antigravity_trust_hint_dismissed INTEGER`
     to `preferences`, with fresh DDL, migration step, and historical-schema tests updated.
   - `checkout_config.rs` and `preferences.rs` wire `LaunchHarness::Antigravity` into
     `GET`/`PUT /api/checkout-agent-trust/antigravity` and `antigravity_trust_hint_dismissed` preference updates.
   - Unlike Codex, Muse and Pi (which remain managed-checkouts-only), Antigravity's switch applies to **all Antigravity
     structured agent launches**, including both managed checkouts and plain/user-chosen folders (New, Clone, Restart
     with, Replace with, agent-requested spawns, and disposable-environment launches). Plain Restart and plain Replace
     still keep the session's stored launch unchanged, and Command launches keep their literal words.
   - When Antigravity's trust switch is enabled, `launches::compile` emits `["--settings", "{antigravity:trusted-cwd}"]`
     for `workspace_trust = Some(true)` and nothing for `Some(false) | None` (no untrusted placeholder).
   - When an Antigravity launch uses Custom permission flags that already pass `--settings` or `--settings=...` (before
     any bare `--`), Farhelm omits its trust flag (`workspace_trust = None`) so the user's `--settings` is never
     overwritten and `LaunchSelection::workspace_trust` (and approval cards) honestly reflect that trust was omitted.
   - `normalize_launch_workspace_trust` and `sessions.rs`'s `Restart with` branch skip supervisor
     `managed_checkout_for_cwd` / `drain_sessions` ownership lookups when the harness does not require a managed
     checkout or trust is blocked by Custom `--settings`.
4. **Desktop GUI Settings switch, discovery hint banner on all Antigravity launches, and goldens.**
   - `TrustAgent::Antigravity` is added to `crates/gui/farhelm-gui/src/settings/agent_trust.rs`, paired with
     `Choice::ShowAntigravityTrustHint` in `settings/choices.rs`, without a caution box (`has_caution() == false`).
   - Settings label, tooltip, help, and banner wording accurately reflect that the Antigravity switch applies to **all
     Antigravity launches** (not only managed checkouts), uses Antigravity's per-launch `--settings` flag without
     editing saved configuration, is off by default because repository hooks, settings and commands are then trusted,
     and notes both vendor caveats (omitted when Custom permission flags already pass `--settings`; `--settings`
     replaces Antigravity's saved trusted-folder list for that session rather than appending to it).
   - `session_pane/trust_band.rs` shows the non-modal discovery banner above the terminal for **any** Antigravity
     structured launch (managed checkout or plain folder) while Antigravity's trust prompt is visible, the switch is
     off, and the Antigravity hint is not dismissed; Claude, Codex, and Muse remain managed-checkout-only. Clicking
     **Settings** opens **Settings → Agent trust** focused on Antigravity's switch; **Don't show again** stores
     `antigravity_trust_hint_dismissed: true` (reversible beside the switch); **×** hides the current occurrence only.
   - Re-capture and visually verify any affected GPUI screenshot goldens (notably `settings-agent-trust` in
     `crates/gui/farhelm-gui/src/goldens/settings.rs`, which opens `Section::AgentTrust` and now renders a 5th trust
     agent and reads a 5th trust route).
5. **Specs, docs, tests, and TODO cleanup.**
   - Update `SPEC.md`, `SPEC_impl.md`, `website/src/content/docs/docs/agents/antigravity.md`,
     `website/src/content/docs/docs/agents/index.mdx`, `website/src/content/docs/docs/using/start-a-session.mdx`, and
     `website/src/content/docs/docs/using/settings.md` to match the new Antigravity trust switch (all launches), Custom
     `--settings` omission, Waiting status on trust prompt, and discovery hint banner.
   - Remove ONLY this plan's TODO.md entry in the final PR (leave the separate "Consider consistent workspace-trust
     scope across agents" Near-term entry untouched).
   - Every added or changed test follows `.agents/test-authoring.md` and passes `scripts/check-test-sleeps.py`.

## Decisions already made

Settled with the maintainer on the 2026-11-20 planning page:

- **Trust switch scope:** All Antigravity launches, including plain folders (not managed checkouts only). A separate
  Near-term TODO item ("Consider consistent workspace-trust scope across agents") tracks deciding whether to align the
  other agents later.
- **Collision with Custom `--settings`:** Omit Farhelm's trust flag when Custom permission flags already pass
  `--settings` (or `--settings=...`), keeping the user's `--settings` intact, and note this in Settings help.
- **Waiting status on trust prompt:** Report `Waiting` while Antigravity's trust prompt is on screen (matching Muse's
  trust-only Waiting behavior), keeping generic Working/Idle for everything else.
- **Review gate:** A single fresh-context `gpt-6.2-sol` agent at high effort with the general review charter.
- **Release note:** Settled verbatim by the maintainer (see `## Release note` below).
- **No-workhorse mode:** The plans default, stated on the planning page.

Planner choices (after fresh-context planning review):

- Keep `launches::workspace_trust_requires_managed_checkout(LaunchHarness) -> bool` (helm) and
  `TrustAgent::managed_checkouts_only(self) -> bool` (GUI) as separate small exhaustive methods rather than sharing a
  flag-only table on `LaunchHarness` in `farhelm-proto` (where `LaunchHarness::Claude` has
  `offers_workspace_trust() == false` even though Claude is managed-only in the UI, and changing any agent's scope later
  already rewrites its UI wording in `agent_trust.rs`). Do not introduce a new proto enum just to hold a two-state
  boolean.
- Three PRs along crate dependency boundaries (`proto + supervisor` → `helm` → `gui + docs`).

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Code rules": per-harness behavior must live in exhaustive matches carrying
  `#[warn(clippy::wildcard_enum_match_arm)]`, never `kind == X`, `matches!`, or `_` over harnesses in shared code.
  Follow the layer map in `crates/farhelm-supervisor/src/agent_kind/mod.rs`.
- `SPEC.md` and `SPEC_impl.md` (Agent trust, Antigravity, Status, Launch, protocol versioning).
- `docs/agent-screen-fixtures.md` (real captured screens, no personal data).
- `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`, and `docs/test-sleep-check.md`.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and `website/AGENTS.md`
  (incidental/unattended docs updates start no preview server).

**Planner proposals** (verified against the code and tightened by the fresh-context planning review):

- **Keep `launches::compile` and `launches::with_workspace_trust` pure.** Do not duplicate the Custom `--settings`
  collision check into `compile` or `with_workspace_trust` (which `yolo_guard::check_agent_wrapper` relies on being pure
  and deterministic, and where suppressing the flag while keeping `selection.workspace_trust == Some(true)` would make
  approval cards display a false `Trusted` chip). Instead, define one exhaustive
  `#[warn(clippy::wildcard_enum_match_arm)]` function
  `launches::workspace_trust_blocked_by_selection(selection: &LaunchSelection) -> bool` in
  `crates/farhelm-helm/src/launches.rs` (true only for `LaunchHarness::Antigravity` when
  `permissions == Some(LaunchPermission::Custom)` and `permission_flags` contain `"--settings"` or a `"--settings="`
  prefix before any bare `"--"`; false for every other harness) and consult it in **one place**:
  `configured_launch_workspace_trust` (`crates/farhelm-helm/src/workspace_trust.rs`), forcing `trust = None` when
  blocked. Keep it separate from supervisor-side `claude::hook_refusal` (different crate and contract).
- **Consolidate trust-resolution orchestration in `workspace_trust.rs`.** Make `configured_launch_workspace_trust` own
  the complete pure formula: `enabled && (managed || !requires_managed_checkout) && !blocked_by_selection`. That
  automatically covers plain-folder disposable-environment launches (`crates/farhelm-helm/src/agent_requests.rs`,
  `ApprovalAction::LaunchEnvironment`) too. Add one small shared helper in `workspace_trust.rs` (e.g.,
  `needs_managed_checkout_lookup` or `normalize_restart_workspace_trust`) so `normalize_launch_workspace_trust` and
  `sessions.rs`'s `Restart with` branch share the exact same gate and skip supervisor ownership lookups
  (`managed_checkout_for_cwd` / `drain_sessions`) when trust does not require a managed checkout or is blocked.
- **Keep `AntigravityReader` trust-only with `has_screen_rules() == false`.** `ticker.rs` increments `fallback_streak`
  only when `has_screen_rules() && !reading.anchored`, so leaving `has_screen_rules() == false` guarantees ordinary
  Antigravity screens never emit spurious drift warnings, while `Reading::anchored(ScreenState::Waiting)` on the trust
  prompt still reports `Waiting` and clears `startup_provisional` on reload.
- **Fit Antigravity fixtures cleanly into `screen_fixtures.rs`.** Commit only the real sanitized
  `antigravity/1.5.13/waiting-trust.{txt,title}` capture (plus a narrow derived variant only if real `agy` actually
  clips or changes wording at narrow width — check with `scripts/agent-tui.py`, scrubbing `ANTIGRAVITY_*` env vars if
  running inside Antigravity, without answering the trust prompt so `~/.gemini/antigravity-cli/settings.json` stays
  untouched). Do not commit an `idle-*` fixture under `tests/fixtures/screens/antigravity/` (where `idle-*` promises
  anchored `ScreenState::Idle`). Update `screen_fixtures.rs` so trust-only harnesses are validated for personal data and
  emulator equivalence without requiring an unimplemented `working`/`idle` activity contract; test negative cases (idle
  prompt, quoted trust text, post-dialog output, wrapped text) inline in `screen_reader.rs`.
- **Keep `TrustBand::Facts` single-field.** In `crates/gui/farhelm-gui/src/session_pane/trust_band.rs`, compute scope
  eligibility once (`!agent.managed_checkouts_only() || row.info.working_copy.is_some()`) for both `at_question` and
  `draw`, passing it into `Facts`'s existing single eligibility field (renaming `managed` to `eligible` or documenting
  it) rather than adding a second boolean field.

## Outline

Three PRs, in this order:

1. `plan/antigravity-workspace-trust/01-proto-and-supervisor` (`feat:`):
   - `farhelm-proto`: `ANTIGRAVITY_TRUSTED_CWD_PLACEHOLDER`, `RESERVED_PLACEHOLDERS`,
     `LaunchHarness::Antigravity.offers_workspace_trust() == true`, `antigravity_trust_hint_dismissed` on `Preferences`
     and `PreferencePatch`, and `PROTOCOL_VERSION` bump (+1).
   - `farhelm-supervisor`: `fill_cwd` / `has_cwd_placeholder` canonical-path JSON substitution for
     `{antigravity:trusted-cwd}`, `AntigravityReader` in `screen_reader.rs` (`Waiting` on trust prompt, generic
     otherwise, `has_screen_rules() == false`), sanitized `antigravity/1.5.13/waiting-trust.{txt,title}` fixture,
     `screen_fixtures.rs` trust-only harness support, and unit + ticker tests.
   - Changelog fragment under `releasing/changelog.d/` (`kind: added`, describing the user-facing feature in progress;
     PR 2 and PR 3 amend this same fragment rather than adding separate entries).
2. `plan/antigravity-workspace-trust/02-helm-trust-wiring` (`feat:`):
   - `farhelm-helm`: next DB schema migration (`checkout_config.antigravity_trust`,
     `preferences.antigravity_trust_hint_dismissed`), `checkout_config.rs` (`/api/checkout-agent-trust/antigravity`),
     `preferences.rs`, `launches.rs` (`workspace_trust_enabled`, `workspace_trust_requires_managed_checkout`,
     `workspace_trust_arguments`, `workspace_trust_blocked_by_selection`), consolidated orchestration in
     `workspace_trust.rs` and `sessions.rs` (`Restart with`), plus store, launch, route, and agent-request tests
     covering managed checkouts, plain folders, Custom `--settings` omission, Restart/Replace preservation, and
     migration.
   - Amend the changelog fragment from PR 1 if needed.
3. `plan/antigravity-workspace-trust/03-gui-and-docs` (`feat:`):
   - `farhelm-gui`: `TrustAgent::Antigravity` (`managed_checkouts_only() == false`, wording for all launches and both
     `--settings` caveats, `has_caution() == false`, `band_words`), `Choice::ShowAntigravityTrustHint`,
     `session_pane/trust_band.rs` scope eligibility update, GUI tests (`settings/tests.rs`,
     `session_pane/trust_band_tests.rs`), and `settings-agent-trust` golden update/verification per
     `.agents/validation.md`.
   - Update `SPEC.md`, `SPEC_impl.md`, and website docs (`docs/agents/antigravity.md`, `docs/agents/index.mdx`,
     `docs/using/start-a-session.mdx`, `docs/using/settings.md`).
   - Finalize the changelog fragment from PR 1 and remove this plan's entry from `TODO.md` (leaving the separate
     "Consider consistent workspace-trust scope across agents" entry in place).

## Fallbacks

- If capturing a fresh Antigravity `1.5.13` trust screen in the executing environment fails (for example, `agy` is not
  installed or not signed in on the executor's host), do not fabricate a fake capture or sign in interactively: block
  per `plans/.agents/executing.md` (step 10) with what failed, or if `agy` is installed and shows the trust screen
  before sign-in or with the host's existing login (observing without answering so `settings.json` stays untouched),
  capture and sanitize it.
- If re-capturing `settings-agent-trust` in PR 3 reveals that `Settings → Agent trust` needs a minor spacing/scroll
  adjustment to fit 5 agents cleanly, adjust it within existing Settings styles and log the choice; do not redesign the
  Settings dialog layout.
- If `PROTOCOL_VERSION` or helm `SCHEMA_VERSION` has moved on `main@origin` by execution time, bump from the current
  values on `main@origin` (and renumber cleanly on rebase per `.agents/careful-rebase.md`).
- If `scripts/agent_screen_host.py`'s `OUTER_SESSION_MARKERS` does not scrub `ANTIGRAVITY_*` when capturing a fixture
  from inside an Antigravity session, scrub them in the capture command's environment (e.g., `env -u ...`) rather than
  expanding PR 1's scope into unrelated harness scripts, unless updating `OUTER_SESSION_MARKERS` is plainly needed and
  logged as a DECISION.

Decisions to log: exact `AntigravityReader::trust_prompt` anchor rules and whether a narrow derived fixture was needed,
exact helper shape consolidating `workspace_trust.rs` and `sessions.rs`, any wording details in `agent_trust.rs`, and
every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-antigravity-workspace-trust-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-antigravity-workspace-trust-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. Run
`python3 releasing/check-changelog.py format` on the changelog fragment, and `dprint check` on changed Markdown files.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/antigravity-workspace-trust/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves or extends the fragment its type calls for, describing in user-facing words
  what was built. Where what got built departs from what this file describes, the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry (leaving the separate "Consider consistent workspace-trust
  scope across agents" entry untouched).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at high effort,
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

Antigravity agents now support automatically suppressing the "trust this directory?" question. Enable it in Settings ->
Agent Trust.

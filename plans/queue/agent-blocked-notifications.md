# Goal: Fix ntfy waiting alerts and add blocked-agent notifications

Fix the traps that prevent waiting-for-answer ntfy alerts from firing reliably, add separate Settings toggles for
waiting-for-answer and blocked-agent alerts (under a master ntfy switch) with optional custom blocked criteria, add
`farhelm agent notify` over the authenticated agent relay (sending ntfy when enabled and recording a desktop rail
Notices bell entry), and inject the locked-in blocked-notification instructions into supported agents on new and
restarted sessions.

This plan depends on `settings-wait-while-saving.md` landing on `main` first (`(after settings-wait-while-saving.md)`).

## Checkout and project identity

This goal runs inside a checkout of `scode/farhelm` ("the checkout the executing session runs in"). Confirm that the
nearest Git worktree or jj workspace containing the current working directory (a `.git` file counts as well as a `.git`
directory; never walk up to an enclosing checkout if the nearest fails) has a remote pointing at `scode/farhelm` (SSH or
HTTPS, with or without `.git`). If not, stop immediately before the resume protocol, write nothing (not even a log), and
report the mismatch. Every project path below is relative to that checkout's root.

## Resume protocol

Right after confirming the checkout, resolve the plan's record directory with
`cargo xtask records path plan/agent-blocked-notifications` (`plans/.agents/outside-files.md`, "Files outside the
repository"), and invoke the `agent-resumeable` skill with `<record-dir>/log.md`. If that log already exists, read all
of it (and anything in `<record-dir>/notes/`) and resume where the previous run left off, cross-checking the log against
reality (VCS state and open `plan/agent-blocked-notifications/*` PRs on GitHub, with GitHub authoritative for what was
pushed). If the log does not exist yet, this is a fresh start.

## Execution mode

No-workhorse mode. Do not send any unit of your own decomposition to a weaker model or lower reasoning effort than your
own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point. If
`scode-galaxy-brain` is already active when this run starts, this goal file is the maintainer's express instruction to
stop it.

Same-model sub-agents at your own model and reasoning effort are fine when they are likely to finish work sooner without
costing quality (at your judgment; concurrent writers may use separate worktrees or jj workspaces whose results you
merge back, while you still own the `jjstack` stack and every PR). Launch sub-agents natively when available, or through
`scode-harness-shellout` when the demanded model is not available as a native sub-agent type.

The sub-agents listed in `plans/AGENTS.md`, "Sub-agents this file requires" (resume check, cold reads of Blocked
questions and reports), the per-PR review gate below, and the scope reassessment review below are exempt from any
no-delegation constraint and run as specified.

## Resource watchdog

Right after the resume protocol and before any delegation, start the `scode-build-goal` resource watchdog so memory and
disk are monitored for the rest of the run:

- **Script path:** resolve `scripts/watchdog` next to the `scode-build-goal` skill's `SKILL.md` with symlinks resolved
  (`realpath`).
- **Status file:** `farhelm-plan-agent-blocked-notifications-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files"), never inside the `plan/agent-blocked-notifications`
  record directory.
- **Watched paths (`--path`):** the root of the executing checkout, the home directory, `/tmp`, the parent directory
  holding the status file, the session's own scratch directory, and any worktree or build directory created on another
  filesystem (restart the sampler with it added). Interval: 60 seconds, with the script's default alert thresholds.
- **Delivery and verification:** follow `scode-build-goal`, "Resource watchdog" for the active harness (Claude Code dual
  Monitors, Codex `skillette-codex-wake-on-exit`, or the plain background sampler fallback when neither exists). Prove
  the mechanism once on throwaway status files in your scratch directory (`--interval 5`, `--synthetic-alert`, kill
  test, `--stall-after 2`) before arming the real monitors, and stop the throwaway runs afterwards.
- **Acting and lifecycle:** record handles, paths, and mechanism in the working log. Run
  `watchdog check --status <file>` before every new workload launch and at natural boundaries. On `ALERT`, `WORSE`, or a
  failed `check`, pause launches and free run-owned space before resuming. On `STALE`, `ERROR`, or monitor exit, pause
  launches until monitoring is restored and `check` passes. Reconcile any still-fresh earlier session's sampler on
  resume before starting yours, and stop your owned monitors when the run ends.

## Goal and acceptance criteria

Deliver reliable ntfy waiting alerts plus comprehensive blocked-agent notifications (command, relay, Notices bell, ntfy
delivery, Settings UI, and per-harness instruction injection).

Acceptance criteria:

1. **Waiting-for-answer ntfy alerts fixed and hardened:**
   - Waiting spells are never consumed (`Spell::Handled`) while waiting alerts are disabled
     (`!enabled || !notify_waiting`).
   - Continuous settle delay is shortened from 10 s to 3 s.
   - Helm logs waiting-alert spell start, settle, suppression/drop, and send outcomes clearly (`tracing::debug!` /
     `tracing::info!` / `tracing::warn!`).
   - Settings → Notifications makes saved-off / unsaved state unmistakably clear (including alongside **Send test** so a
     passing test while alerts are off or unsaved cannot mislead the user into thinking real alerts are active).
2. **Comprehensive ntfy alert settings & explicit default-vs-custom tracking:**
   - Settings → Notifications → ntfy alerts has a master **Enable ntfy alerts** switch plus two event switches:
     - **Agent is waiting for an answer** (`notify_waiting`, default `true` for existing configs so upgrades preserve
       waiting alerts).
     - **Agent is blocked** (`notify_blocked`, default `false`).
   - Under **Agent is blocked**, the user can optionally supply custom **"what counts as blocked"** criteria:
     - Stored explicitly as `Option<String>` (`None` = built-in default, `Some(text)` = user custom criteria; empty or
       whitespace-only input normalizes to `None`). Future changes to Farhelm's default wording automatically apply to
       every `None` config without comparing strings against past defaults.
     - When `custom_blocked_criteria` is `None`, Settings **visibly shows Farhelm's built-in default criteria wording
       read-only**, while keeping the custom criteria text field empty (never pre-filling the editable text box with a
       copy of the default). When customized (`Some`), Settings offers a clear action to reset back to default.
   - Settings search indexes every new switch and field.
3. **GUI derives supported harnesses from agent-kind interfaces in code:**
   - Exhaustive `#[warn(clippy::wildcard_enum_match_arm)]` capability methods on `AgentKind` (in `farhelm-proto`,
     delegated from `LaunchHarness::agent_kind()` and pinned against supervisor `delivers_instructions` and
     `screen_reader` by unit tests) report which agents support instruction injection and which support
     waiting-for-answer alerts (`QuestionsAndApprovals | WorkspaceTrustOnly | None`).
   - Settings UI builds its supported-agent wording dynamically from `LaunchHarness::BUILT_INS` and those capability
     methods — never from manually maintained prose lists.
4. **Agent notification command (`farhelm agent notify`):**
   - `farhelm agent notify "<reason>"` sends `AgentVerb::Notify { reason }` over Farhelm's existing authenticated
     supervisor→helm relay (`FARHELM_SUPERVISOR_SOCK`, `SessionAuth`), working identically on local hosts, remote SSH
     hosts, Tensorlake sandboxes, and Podman containers without exposing ntfy credentials or requiring outbound network
     access from the session.
   - Requires no approval card, authorizes self-session only, validates and bounds `reason` (non-empty after trim, max
     500 chars, presentation-unsafe characters stripped), and replies with `AgentReply::Notified {}`.
   - Always records a host-scoped notice in the desktop rail Notices bell (`crates/farhelm-helm/src/notices.rs` +
     `crates/gui/farhelm-gui/src/rail_notices.rs`) with the session title and reason (mark-read resolves it; repeated
     calls from the same session replace with a fresh notice ID so repeat alerts are never dropped and `helm.db` stays
     bounded to at most 1 row per session).
   - Enqueues an ntfy alert when `enabled && notify_blocked && !topic.is_empty()`, reusing the helm's existing ntfy
     sender, `clean_text()`, and 5/min `RateWindow`.
   - Automatically appears in `farhelm agent instructions` via clap introspection.
5. **Exact locked-in blocked-agent instruction injection:**
   - When `enabled && notify_blocked`, the helm resolves the exact blocked-agent instruction text (from one shared
     helper in `farhelm-proto/src/ntfy.rs`) and passes it as launch-time `extra_instructions: Option<String>` on
     `ControlMsg::CreateSession` and `ControlMsg::RestartSession` so **new sessions and Restarted sessions** receive the
     current setting without storing it on `SessionLaunch` or in supervisor SQLite.
   - Exact wording injected (single source of truth in `crates/farhelm-proto/src/ntfy.rs`):
     - Framing (always injected when enabled):
       ``farhelm: If you are going to stop without completing what the user asked because you are blocked, do not stop silently — run \``farhelm
       agent notify "<what is blocked (briefly)>: <why is it blocked (briefly)>"\` before ending your turn so the user
       is notified right away. Do not treat this as a request to try harder or work around blockers; only make sure you
       notify instead of stopping quietly. Keep the reason to one short sentence saying what blocked you and what you
       need.`
     - Default criteria (appended when `custom_blocked_criteria` is `None`):
       `What counts as blocked: you cannot finish what the user asked because of missing credentials, permissions, tools, or access; a decision or clarification only the user can make; an external failure or environment problem you cannot resolve; or any case where you are leaving requested work unfinished or skipping part of the request.`
     - Custom criteria (appended when `custom_blocked_criteria` is `Some(user_text)`):
       `What counts as blocked: <user_text>`
   - Delivered across all 6 harnesses that currently support instruction injection (Claude, Codex, Muse, Goose, Pi,
     OMP), respecting `FARHELM_AGENT_INSTRUCTIONS=off`, and keeping Claude/Codex `SessionStart` hook stdout plain text
     (never starting with `{` and ending with `}`).
   - Verify during implementation/testing that Muse's agent shell/bash tool children still inherit `FARHELM_SESSION_ID`,
     `FARHELM_SESSION_TOKEN`, and `FARHELM_SUPERVISOR_SOCK` so Muse can execute `farhelm agent notify`.
6. **Specs, tests, goldens, changelog fragment, and `TODO.md`:**
   - `SPEC.md` and `SPEC_impl.md` are updated for the 3 s settle time, new ntfy settings/controls,
     `farhelm agent notify`, blocked instruction injection, and `AgentBlocked` rail notices.
   - Unit, integration/e2e, and GUI tests (including `settings/tests.rs`, `rail_notices` tests, `ntfy_tests.rs`,
     protocol/schema migration tests, and affected screenshot goldens) pass.
   - The `TODO.md` entry **"Inject instructions to notify the user when blocked."** is removed in the last code PR.

## Decisions already made

### Requirement source 1: User request and settled decisions

- **Original TODO entry:** _"Inject instructions to notify the user when blocked. Provide way to inject instructions for
  agents to notify the user when blocked through farhelm."_
- **Maintainer picker note:** Observed that Send test worked while a real red/waiting session produced no ntfy alert;
  requested comprehensive ntfy control with separate toggles for "agent is waiting for a question answer" and "agent is
  blocked", strong agent instruction injection so an agent does not stop silently when unable to finish what the user
  asked (the 2-hour unattended failure case), and optional custom "what does it mean to be blocked" criteria wording.
- **Settled waiting-alert fixes:** Fix disabled-spell consumption trap, shorten settle to ~3 s, add diagnostic logging,
  and clarify unsaved/off state in Settings (including on Send test).
- **Settled toggle structure:** Master **Enable ntfy alerts** switch + two event toggles (**Agent is waiting for an
  answer** and **Agent is blocked**).
- **Settled agent command & surface:** Add `farhelm agent notify` via the supervisor→helm relay; sends ntfy (when
  enabled) AND records in the desktop rail Notices bell.
- **Settled harness scope & code-driven UI:** Global setting for the 6 harnesses that currently support instruction
  injection (no expansion to Grok/Antigravity/Cursor/OpenCode/Custom in this plan); the GUI MUST use actual agent-kind
  interfaces in code to determine which kinds support each feature, never manually maintained prose lists.
- **Settled wording, framing intent, and default-vs-custom tracking:**
  - Goal is NOT to make the agent "try harder" or change its behavior except not to stop silently.
  - Exact locked-in wording as stated in Acceptance criterion 5 above (including
    `"<what is blocked (briefly)>: <why is it blocked (briefly)>"`).
  - Explicitly track whether the user picked custom criteria vs built-in default (`Option<String>`: `None` vs `Some`),
    never by comparing strings against past default strings.
  - Settings UI MUST visibly show the built-in default wording when not using custom settings.
- **Settled instruction update timing:** New sessions and Restarted sessions (helm-side ntfy delivery updates
  immediately on Save).
- **Settled queue ordering:** `(after settings-wait-while-saving.md)`.
- **Settled review gate:** Fresh-context **Opus 5.5** agent at **high** effort with the general review charter.
- **Settled release note:** Recorded verbatim in `## Release note` below.

### Requirement source 2: Binding repository constraints

- `AGENTS.md`, "Code rules", and `crates/farhelm-supervisor/src/agent_kind/mod.rs`: all per-harness capabilities live in
  exhaustive `#[warn(clippy::wildcard_enum_match_arm)]` matches or required `AgentIntegration` methods — never
  `kind == X`, `matches!` over specific harnesses, or `_` arms in shared code.
- `SPEC.md` ("Ntfy alerts", "Instructions pointer", "Settings → Notifications") and `SPEC_impl.md` ("Ntfy waiting
  alerts", "The instructions pointer"): keep DB/network awaits off the `HostActor` refresh path (`SPEC_impl.md:6228`);
  `SessionStart` stdout on Claude/Codex must never start with `{` and end with `}` (`crates/farhelm/src/hook.rs`);
  `PROTOCOL_VERSION` bumps cleanly from 91 to 92 (`crates/farhelm-proto/src/lib.rs`, noting `.agents/careful-rebase.md`
  if main bumps first).
- `AGENTS.md` ("Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation"),
  `.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`.
- Note on `releasing/changelog.d/`: a protocol version bump alone gets no release-note entry, and no entry says remote
  hosts must be updated (`AGENTS.md`, "Changelog fragments").

### Requirement source 3: Planner-proposed implementation choices (scope-reviewed)

These choices came from repository inspection and a fresh-context planning scope review; preserve them unless reality in
the checkout requires a documented `DECISION` or scope reassessment:

1. **Proto & single source of truth (`crates/farhelm-proto/`):**
   - Extend `ntfy::Settings` and `ntfy::SaveSettings` (`crates/farhelm-proto/src/ntfy.rs`) with backward-compatible
     `#[serde(default)]` fields: `notify_waiting: bool` (default `true`), `notify_blocked: bool` (default `false`), and
     `custom_blocked_criteria: Option<String>` (default `None`). Use plain `Option<String>` on `SaveSettings` (no
     tri-state enum; `PUT /api/ntfy-alerts` is a full-form replacement of readable fields, normalizing empty or
     whitespace-only input to `None`, collapsing line breaks to spaces, rejecting/stripping presentation-unsafe/control
     chars, max 1,000 chars).
   - Place `BLOCKED_INSTRUCTION_FRAMING`, `DEFAULT_BLOCKED_CRITERIA`, and
     `blocked_instructions(&Settings) -> Option<String>` in `crates/farhelm-proto/src/ntfy.rs` so helm and GUI share one
     exact source of truth.
   - Define the two `#[warn(clippy::wildcard_enum_match_arm)]` capability methods **once on `AgentKind`** (beside
     `AgentKind::offers_persistent_hooks` in `crates/farhelm-proto/src/lib.rs`):
     - `AgentKind::supports_instruction_injection(self) -> bool`
     - `AgentKind::waiting_alert_support(self) -> WaitingAlertSupport`
       (`QuestionsAndApprovals | WorkspaceTrustOnly | None`) Have `LaunchHarness` delegate via `self.agent_kind()`, and
       pin both methods with unit tests in `farhelm-supervisor` against `delivers_instructions` and
       `screen_reader::reader_for`.
   - Add `AgentVerb::Notify { reason: String }` (`is_mutating() == true`, `may_wait_for_user() == false`,
     `may_launch_environment() == false`) and `AgentReply::Notified {}` (matching existing tagged empty variants; bump
     `PROTOCOL_VERSION`).
   - Add `#[serde(default, skip_serializing_if = "Option::is_none")] extra_instructions: Option<String>` directly on
     `ControlMsg::CreateSession` and `ControlMsg::RestartSession` (matching `CreateSession::prompt` and
     `RestartSession::setup_persistent_hooks`). Do **not** put it on `SessionLaunch`, do **not** add a supervisor SQLite
     migration, and exclude it from `CreateSession`'s `intent_key` fingerprint.
   - Add `NoticeContent::AgentBlocked { session_id: String, session_title: String, reason: String }` in
     `crates/farhelm-proto/src/notices.rs`.
2. **Helm ntfy fixes, relay handling, and Notices bell (`crates/farhelm-helm/`):**
   - Cache `waiting_alerts_enabled: Arc<AtomicBool>` on `HelmStore` (initialized in `HelmStore::open` and updated inside
     `HelmStore::save_ntfy_config`) so `HostActor::serve` passes `self.store.waiting_alerts_enabled()` synchronously
     into `alert_policy.observe(&observations, now, waiting_enabled)` with zero DB awaits on the refresh path.
   - In `Policy::observe`: while `!waiting_enabled`, clear `self.spells` and set `self.initialized = false` (so nothing
     is consumed while disabled, and enabling re-baselines stale already-waiting sessions cleanly while every new
     waiting spell alerts after `SETTLE = Duration::from_secs(3)`). Add `tracing::debug!` / `tracing::info!` logs on
     spell start, settle, disabled/topic/host drops, and successful sends.
   - Generalize `ntfy::Alert` with `enum AlertKind { Waiting, Blocked { reason: String } }`, with `enqueue_waiting` and
     `enqueue_blocked`, reusing the bounded queue, `reqwest::Client`, `post()`, `clean_text()`, host display-name
     resolution, and shared 5/min `RateWindow`. Gate `Waiting` on `enabled && notify_waiting && !topic.is_empty()` and
     `Blocked` on `enabled && notify_blocked && !topic.is_empty()`.
   - Extend `crates/farhelm-helm/src/notices.rs` + `store.rs` with one `rebuild_notices` schema migration adding
     `'agent_blocked'` to `notices.kind`, resolving on `MarkAction::Read` (like `late_delete`), and writing with
     `INSERT OR REPLACE` keyed by `(host_id, 'agent_blocked', session_id)` (like `raise_new_announcement_notice`) so
     repeated blocked alerts mint fresh IDs without silently dropping or bloating `helm.db`.
   - Handle `AgentVerb::Notify { reason }` in `HelmAgentRequests::handle`: self-session only, validate/sanitize/cap
     `reason` (non-empty after trim, max 500 chars), look up session title via existing
     `crate::approvals::describe_session`, unconditionally record the `AgentBlocked` notice in `notices.rs` (and bump
     feed events), enqueue the ntfy blocked alert (gated in the sender task on
     `enabled && notify_blocked && !topic.is_empty()`), and return `AgentReply::Notified {}`.
   - Resolve `extra_instructions` from `store.ntfy_config()` in `do_create_session_with_environment` and
     `do_restart_session_named` (`crates/farhelm-helm/src/sessions.rs`).
3. **CLI and Supervisor inline instruction delivery (`crates/farhelm/`, `crates/farhelm-supervisor/`):**
   - Add `AgentCmd::Notify { reason: String }` (`farhelm agent notify <reason>`) calling `agent_client::relay_request`,
     automatically picked up by `farhelm agent instructions`.
   - Keep `AgentInstructions` as a `Copy` (`On | Off`) enum; add `pub extra_instructions: Option<&'a str>` to
     `HookPolicy<'a>` (`crates/farhelm-supervisor/src/agent_kind/mod.rs`). Gate `extra_instructions` behind
     `policy.instructions.announces()` so `FARHELM_AGENT_INSTRUCTIONS=off` suppresses all injected instructions and
     `Stop` hooks never emit them.
   - Deliver `extra_instructions` inline with **no state-dir instruction files and no `LaunchArtifacts` changes**
     (keeping `AgentIntegration::hook_argv` pure and surviving delayed/repeated `SessionStart` hooks):
     - **Claude / Codex / Muse:** optional `--extra-instructions <TEXT>` flag on `farhelm internal hook` in
       `hook_command` (shell-quoted via `farhelm_proto::text::shell_quote`, then JSON/TOML encoded; survives Muse's hook
       env scrubbing). Print `POINTER_LINE` followed by extra instructions as plain text.
     - **Goose:** `FARHELM_GOOSE_EXTRA_INSTRUCTIONS` in `FarhelmArgs.env` (scrubbed alongside
       `FARHELM_GOOSE_INSTRUCTIONS` in `launch.rs`), joined with `POINTER_LINE` in `InternalCmd::GooseHook` ->
       `goose_hook::serve`.
     - **Pi / OMP:** combine `INSTRUCTIONS_POINTER` and `extra_instructions` into the single existing
       `--append-system-prompt` argument (preserving OMP's last-wins behavior).
   - Take `extra_instructions` from incoming `CreateSession` on unconfirmed-create retry recovery
     (`crates/farhelm-supervisor/src/service/core.rs`).
   - Verify Muse's agent shell/bash tool children still inherit `FARHELM_SESSION_ID`, `FARHELM_SESSION_TOKEN`, and
     `FARHELM_SUPERVISOR_SOCK` so Muse can run `farhelm agent notify`.
4. **Desktop GUI (`crates/gui/farhelm-gui/src/settings/ntfy.rs`, `settings/search.rs`, `rail_notices.rs`):**
   - Extend `NtfyAlerts`'s `Draft<Form>` with `notify_waiting`, `notify_blocked`, and `custom_blocked_criteria` (using
     existing `TextInputState::new_growing` — no new textarea widget), plus Settings search entries.
   - Dynamically generate supported-agent help text from `LaunchHarness::BUILT_INS` via
     `AgentKind::supports_instruction_injection` and `AgentKind::waiting_alert_support`.
   - When `custom_blocked_criteria` is `None`, render `DEFAULT_BLOCKED_CRITERIA` visibly as read-only text beside the
     empty custom input; when non-empty, provide a clear Reset-to-default control.
   - Surface clear inline status when alerts are saved-off or have unsaved edits (including beside **Send test**).
   - Render `NoticeContent::AgentBlocked` in `crates/gui/farhelm-gui/src/rail_notices.rs` (the desktop rail Notices
     bell, NOT `notices_bell.rs`).
5. **Suggested PR stack (3–4 cohesive, reviewable PRs):**
   - **PR 1:** Proto + capability methods (`ntfy::Settings` extensions, locked-in wording & helper, `AgentKind`
     capability methods + supervisor pin tests, `AgentVerb::Notify` / `AgentReply::Notified`,
     `NoticeContent::AgentBlocked`, `ControlMsg` `extra_instructions`, protocol bump).
   - **PR 2:** Helm ntfy waiting-alert fixes (3 s settle, `waiting_alerts_enabled` atomic cache + `Policy` reset,
     logging), blocked ntfy sender, `AgentBlocked` notice migration + `HelmAgentRequests::handle`, and `CreateSession` /
     `RestartSession` instruction wiring.
   - **PR 3:** `farhelm agent notify` CLI + supervisor inline instruction delivery across all 6 harnesses (Claude,
     Codex, Muse, Goose, Pi, OMP) + e2e tests (including Muse env inheritance check).
   - **PR 4:** Desktop Settings → Notifications UI + search + `rail_notices.rs` rendering + GUI tests/goldens +
     `SPEC.md` / `SPEC_impl.md` updates + `releasing/changelog.d/` fragment + `TODO.md` entry removal.

## Scope reassessment

Before implementing a substantial departure from the outline above (for example, introducing state-dir instruction
files, persisting instructions in supervisor SQLite, expanding instruction injection to additional harnesses, or adding
a new notice subsystem), or after repeated corrective reviews of the same component, run a fresh-context review at your
own model and effort using this exact charter:

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

Supply the original request, settled decisions, outline, current diff, and proposed departure (what changed, why it is
necessary, and which simpler alternative was ruled out). A close reuse-versus-duplication call that stays within agreed
scope is a routine fork: decide it after reassessment and log it as `DECISION`. If a sub-agent discovers an unapproved
new mechanism mid-unit, leave it unimplemented and report it rather than inventing it to satisfy a checkpoint.

## PR discipline and validation

- Base the stack on `main@origin` (or on this plan's own open `plan/agent-blocked-notifications/*` PRs when resuming,
  never another plan's PRs), set up per `plans/.agents/executing.md`, "Executing one plan". Wait until
  `settings-wait-while-saving.md` has landed on `main` (enforced by `(after settings-wait-while-saving.md)`).
- Use `jjstack` with bookmark naming `plan/agent-blocked-notifications/<nn>-<short-name>`. PRs stay drafts; never mark a
  PR ready and never merge. Restructure an existing open PR of this plan rather than stacking a fixup PR on top.
- Never edit `plans/`.
- Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments" (add a fragment under `releasing/changelog.d/` for
  the user-facing work), and "Tests and validation", plus `.agents/validation.md`, `.agents/test-evidence.md`, and
  `.agents/test-authoring.md`. Remove the covered `TODO.md` entry in the last code PR.

## Review gate

Before finishing any PR in this plan, delegate a review of its changes to a single fresh-context **Opus 5.5** agent at
**high** reasoning effort (explicit demand for this model and effort; launch natively or via `scode-harness-shellout` —
do not invent a custom shell command).

Give the reviewer the checkout root (resolved at run time), the commit range or bookmark to review, the private findings
file path in your scratch directory, `.agents/test-authoring.md` verbatim whenever tests or fixtures changed, and this
complete charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved. Report findings to the named findings file, most
> severe first, with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no VCS
> state.

Address every valid finding and re-verify before moving on.

## Decision logging

Log major design decisions in the plan's working log as they happen with a scannable `DECISION` label, including the
alternatives considered.

## Unattended fallback

This plan executes unattended. Never ask the maintainer anything in conversation — not a direct question, not a harness
question prompt, not an optional offer or closing "should I...?" (`plans/AGENTS.md`, "Unattended means no questions").

- Resolve routine implementation forks within the agreed scope and log consequential choices as `DECISION`. Remove
  unnecessary planner-invented machinery when the requested behavior and repository constraints still hold.
- When remaining work requires a decision only the maintainer can make (a material scope expansion, weakened guarantee,
  or omitted required behavior not covered by settled decisions), push everything consistent and block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking).
- Put things the maintainer should know and optional follow-up ideas into the closing report rather than stopping or
  asking.

## Done criterion

The goal is achieved when every PR in the stack exists as a draft PR on GitHub, passes its review gate and chosen
validation, satisfies the acceptance criteria above (including `TODO.md` entry removal), and satisfies the latest
`## Decisions` entry in this file, if any. Delivering the report is the executor's closing step, not part of the goal.

## Release note

**You can now get ntfy alerts when an agent gets blocked, alongside more reliable waiting-for-answer alerts.** In
Settings → Notifications, enable waiting and blocked alerts separately, optionally describe what counts as blocked, and
Farhelm will instruct supported agents to notify you when they cannot finish what you asked.
